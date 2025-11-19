status: DONE
# Goal 4d – NodeId Format Simplification

Author: coding agent spec written 2024-XX-XX  
Source context: `engine/baml-runtime/src/control_flow.rs` (see `background2.md` for current architecture)

## Background
- Control-flow graphs are produced per function by `build_from_hir` and consumed by the mermaid renderer plus runtime tracing UIs.
- Every `Node` is currently identified by a `NodeId` that encodes the lexical path (`function|hdr:foo:1|arm:then:0|…`). The graph builder recreates these IDs via `NodePathCursor`.
- Downstream UI components only need a stable opaque identifier for graph plumbing. The lexical string is still needed elsewhere (matching trace events, debugging), but it should not be entangled with the `NodeId` equality/hash semantics.
- The existing implementation conflates the two concerns: the `NodeId` struct owns both the opaque ID and the lexical path segments, so accidental mutations break hashing and parent lookups.

## Problem Statement
`NodeId` should be a trivial, opaque handle. Carrying path-segment state inside `NodeId` makes equality/parent derivation rely on complex slug/counter logic, and the traversal currently depends on `NodeId::parent()` to re-derive hierarchy. This makes IDs fragile whenever we want to adjust lexical paths. We still need to retain the lexical path information per node, but it should move to a dedicated `Node.lexical_id` string so we can change either independently.

## Requirements
1. `NodeId` must become a simple newtype wrapper around a monotonically increasing integer (`u32` is enough). Its `encode`/`FromStr` implementations should serialize to/from the plain integer string (e.g., `"42"`). No path data should live inside `NodeId`.
2. `Node` gains a new public field `lexical_id: String`. This must store the same encoded path string we currently get from `encode_segments(function, segments)`, so external tooling can continue to understand lexical ancestry without touching `NodeId`.
3. The traversal/builder must allocate NodeIds centrally (single counter) and pass parent IDs explicitly when constructing nodes. We should no longer rely on `NodeId::parent()`.
4. Remove `NodePathCursor`. Store the lexical `PathSegment` metadata directly on each `Frame` entry so we can rebuild the full path from the active frame stack whenever we need it.
5. Existing helpers such as `encode_segments` remain available (possibly renamed to make their purpose clearer), because they are now used strictly for lexical strings.
6. All call sites (graph builder, mermaid renderer, tests) compile against the new signatures. Snapshot utilities should include the new `lexical_id` field to make it visible in fixture output.
7. Once refactored, there must be no remaining logic that attempts to decode a lexical path back into a `NodeId`. Parsing is only needed for tests/debugging via `NodeId::from_str`, and that now just parses an integer.

## Detailed Implementation Plan

1. **Refactor `NodeId` (engine/baml-runtime/src/control_flow.rs)**
   - Replace the existing struct definition with `pub struct NodeId(u32);` (derive `Copy` + `PartialEq/Eq/Hash` etc.).
   - Provide `impl NodeId { pub fn new(raw: u32) -> Self; pub fn encode(&self) -> String { self.0.to_string() } pub fn raw(&self) -> u32; }`.
   - Delete `NodeId::parent`. Consumers must pass parent IDs explicitly.
   - Update `impl fmt::Display` to delegate to `encode()`, and `impl FromStr` to parse the decimal string into a `u32`.
2. **Add `lexical_id` to `Node`**
   - Extend the struct with `pub lexical_id: String`.
   - Update `Node::new` to accept both the `parent_node_id` and `lexical_id` explicitly: `fn new(id: NodeId, parent: Option<NodeId>, lexical_id: impl Into<String>, label: impl Into<String>, span: Span, node_type: NodeType)`.
   - Update `Node::root` to take a lexical string too (generated from the cursor when the root scope is pushed) and to set `parent_node_id` to `None`.
3. **Introduce a NodeId allocator inside `ControlFlowVizBuilder`**
   - Add `next_node_id: u32` field initialized to `0`.
   - Add helper `fn allocate_id(&mut self) -> NodeId` that returns `NodeId(self.next_node_id)` and increments the counter.
   - Make it accessible to `HirTraversalContext` (e.g., add a thin wrapper method or expose via `ctx.graph.allocate_id()` before calling `add_node`).
4. **Inline lexical-path tracking into the frame stack**
   - Delete `NodePathCursor`.
   - Extend `Frame` with a new field (e.g., `lexical_segment: Option<PathSegment>`) storing the segment that introduced that frame. The root frame stores `PathSegment::FunctionRoot { ordinal: 0 }`.
   - Add helper(s) on `HirTraversalContext` to assemble the lexical-id string by walking the current frame stack, collecting each frame’s stored segment, appending the new child’s segment, and passing all segments to `encode_segments(function_name, &segments)`.
5. **Update traversal code to pass IDs + parents explicitly**
   - Wherever we previously called `let node_id = self.cursor.push_*...;`, split the operation:
     1. `let node_id = self.graph.allocate_id();`
     2. `let lexical_id = self.cursor.push_*...;`
     3. Determine the parent by inspecting the current frame stack (`self.frames[parent_index].node_id.clone()`).
   - Construct nodes via the new `Node::new` signature, passing the parent (if any) and lexical id.
   - When pushing new frames (headers, loops, branch groups, etc.), store the freshly allocated `NodeId` in the frame so later children can use it as their parent reference.
   - `Frame::new` continues to take a `NodeId`, no change besides adapting call sites.
6. **Remove stale helpers**
   - Delete `decode_segments` and any `FromStr` logic for path-based IDs. Keep `encode_segments` (consider renaming to `encode_lexical_path` for clarity) because we still need the slug format for `lexical_id`.
   - Clean up unused enums/structs if any were only needed for the old NodeId type (e.g., if `PathSegment` was only used there, but we still use it inside the cursor so it should stay).
7. **Adjust downstream consumers**
   - `mermaid.rs` only relies on `node.id`, `node.parent_node_id`, and `node.label`; no logic change, but the compiler will force the new `Node::new` usage to set parents.
   - Snapshot helper (`viz_snapshot` in `control_flow/tests.rs`) should add `"lexical_id": node.lexical_id` to each node entry so regressions become visible.
   - Any other module importing `NodeId::encode`/`from_str` must be updated to the new semantics (search for `.encode()` usage; currently only the control_flow tests encode IDs).

## Validation
1. Unit tests: `cargo test -p baml-runtime control_flow::tests`.
2. Snapshot refresh (if needed): rerun the insta suite as described in `background2.md` (`INSTA_FORCE_PASS=1 cargo test -p baml-runtime control_flow::tests`).
3. (Optional) Run the mermaid sandbox build if convenient to visually confirm the graph still renders (not required unless the UI starts depending on lexical IDs).

## Non-Goals / Notes
- Do **not** change how slugs/ordinals are computed; only move the lexical string into `Node.lexical_id`.
- Keep the existing node/edge insertion order semantics so existing snapshots only change in the ID/lexical fields.
- No API changes outside `engine/baml-runtime/src/control_flow.rs` other than necessary updates to compile (e.g., due to constructor signature changes).

Following the steps above should give us a deterministic, opaque `NodeId` plus an explicit lexical path for debugging/tracing.
