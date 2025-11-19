# Control-Flow Visualization Background (Current Implementation)

This note documents how the current control-flow visualization works under `engine/baml-runtime/src/control_flow.rs` (a.k.a. `engine/BAML-runtime/source/control_flow`). It should give another coding agent enough context to reason about the algorithm, its helper passes, and the surrounding tooling before attempting fixes.

## Source Files Involved
- `engine/baml-runtime/src/control_flow.rs` — core graph builder that walks HIR and produces a `ControlFlowVisualization`.
- `engine/baml-runtime/src/control_flow/flatten/*.rs` — three-pass “flattening” pipeline that prunes implicit nodes, expands branch edges, and re-parents scopes.
- `engine/baml-runtime/src/control_flow/mermaid.rs` — renders a `ControlFlowVisualization` (or the flattened result) into Mermaid flowchart text.
- `engine/baml-runtime/src/control_flow/tests.rs` plus `testdata/*.baml` and `snapshots/*.snap` — insta snapshot tests that cover both the raw builder and every flattening stage.
- `engine/baml-runtime/src/control_flow/mermaid-sandbox` — a small React playground that consumes serialized graphs for manual inspection; feed it fresh artifacts generated via `INSTA_UPDATE=always cargo test -p baml-runtime control_flow::tests`.

## Functional Goals
1. Produce a graph for one BAML function (expression or LLM) whose nodes/edges can be rendered directly or piped through the flattening passes.
2. Ensure every node carries two identifiers: a numeric `NodeId` (stable within one traversal) and a string `lexical_id` (`function|segment|…`) so UI layers can correlate nodes with runtime traces or diffs.
3. Represent structural constructs explicitly (headers, branch groups/arms, loop headers, synthetic block scopes) while omitting mundane statements that do not influence header playback.
4. Keep spans on all nodes (`Span::fake` fallback) so downstream tools can still highlight source regions.
5. Provide post-processing hooks (flattening passes + Mermaid rendering) that massage the raw tree for consumption in React Flow and snapshot tests.

## Data Model (control_flow.rs)

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(u32);

#[derive(Clone, Debug)]
pub struct Node {
    pub id: NodeId,
    pub parent_node_id: Option<NodeId>,
    pub lexical_id: String,
    pub label: String,
    pub span: Span,
    pub node_type: NodeType,
}

#[derive(Clone, Debug)]
pub enum NodeType {
    FunctionRoot,
    HeaderContextEnter,
    BranchGroup,
    BranchArm,
    Loop,
    OtherScope,
}

#[derive(Clone, Debug, Default)]
pub struct ControlFlowVisualization {
    pub nodes: IndexMap<NodeId, Node>,
    pub edges_by_src: IndexMap<NodeId, Vec<Edge>>,
}
```

- `NodeId` is now a simple incrementing counter. Stability across edits comes from `lexical_id`, not the numeric value.
- `lexical_id = encode_segments(function_name, &[PathSegment…])` where `PathSegment` encodes `FunctionRoot/Header/BranchGroup/BranchArm/Loop/OtherScope` with slugified labels + ordinals.
- `ControlFlowVizBuilder` hands out node IDs, stores nodes, collects raw edges, then groups them by `src` in `finish()`.

## Traversal State (`HirTraversalContext`)

```rust
struct HirTraversalContext {
    function_name: String,
    graph: ControlFlowVizBuilder,
    frames: Vec<Frame>,
}

struct Frame {
    entry: FrameEntry,
    node_id: NodeId,
    lexical_segment: Option<PathSegment>,
    counters: FrameCounters,
    last_linear_child: Option<NodeId>,
}
```

- A `Frame` mirrors the lexical stack. `FrameEntry` variants are `FunctionRoot`, `Header { level }`, `BranchGroup`, `BranchArm`, `Loop`, and `OtherScope`.
- Each frame keeps independent `FrameCounters` (ordinals) for headers, branch groups/arms, loop nodes, and “other scopes”. Ordinals feed `PathSegment` so lexical IDs remain stable even when statements are inserted earlier in the block.
- `lexical_segment` lets `build_lexical_id` recreate the full path (`function|hdr:slug:ordinal|…`) whenever a child node is emitted.
- `last_linear_child` remembers the most recently emitted direct child so sequential nodes can be linked automatically. `FrameEntry::children_are_linear` returns `false` for `BranchGroup`, which prevents the raw builder from linking the group directly to each arm—those fan-out edges are added later by the flatten pass.

## Entry Points
- `build_from_hir(hir, function_name)` selects either an expr or LLM function and forwards to `build_function_graph`.
- `build_llm_function_graph` is trivial: create a FunctionRoot node + one `OtherScope` node labeled `LLM client: <client>`, connect root → client, done.
- `build_expr_function_graph` instantiates `HirTraversalContext`, pushes the root frame, then calls `visit_function_body`.

## Statement & Expression Handling
- Only structural statements emit nodes. Plain `Declare`, `Break`, `Continue`, and watch statements are no-ops.
- `HeaderContextEnter` drives `enter_header`:
  - Normalizes the level (`max(1)`), pops higher-level headers via `pop_headers_to_level`, allocates a new header node, chains it to the previous linear child, and pushes a new frame so nested statements inherit the header scope.
- `Let`/`Assign`/`Expression` statements determine whether the RHS is a `{ ... }` block expression:
  - `BlockHandling::wrap(Some(label))` wraps the block body inside an `OtherScope` node (with optional label such as `let foo = { ... }`), so the traversal can descend without inventing another header.
  - Inline behavior simply walks the block statements in-place without creating a wrapper node.
- Loops (`While`/`For`/`CFor`) call `visit_loop` with a `LoopFlavor`:
  - Compute a slug/ordinal, emit a `NodeType::Loop` node labeled `while (<expr>)`, `for (id in <expr>)`, or `for (<expr>)`.
  - Push a `Loop` frame, walk the block inline, then pop the frame. The raw builder only links the loop node into the parent chain; there are no explicit back-edges today.
- `visit_expression` only special-cases `If` and `Block`. Every other expression type produces no nodes; it just influences labels/spans used by its parents.
- `visit_if` emits a `BranchGroup` node for the entire conditional, then iteratively emits `BranchArm` nodes for the `then`, chained `else if`, and `else` arms. Each arm is treated like a mini block (with its own frame) so nested headers stay under that arm.

## Header Stack Management
- `enter_header` slugifies header titles, falls back to `header-{ordinal}` when empty, and uses `lexical_segment = PathSegment::Header`.
- `pop_headers_to_level(level)` tears down header frames deeper than the requested level whenever a shallower header is encountered.
- Because headers are the only source of meaningful labels in downstream playback, subsequent passes try hard to keep them near the top of the graph.

## Edge Semantics
- `register_child_with_parent(parent_index, child_id)` adds an edge from the parent’s `last_linear_child` to the new child whenever the parent’s `FrameEntry` allows linear ordering. The parent frame then remembers the new child.
- For branch groups, linear edges are skipped. Instead, `hoist_branch_arms` (flatten pass 2) will:
  1. Replace the parent’s outgoing edges with edges to each `BranchArm`.
  2. Propagate the branch group’s successors to each arm so the flow rejoins naturally.
- Since loops/headers/scopes are linear, the raw builder already threads edges through them.

## Flattening Pipeline (`control_flow/flatten`)
`flatten_control_flow` runs the three passes defined in `flatten/mod.rs`. Tests snapshot each stage separately.

1. **`remove_implicit_nodes`** — drops nodes that neither are headers nor contain headers in their subtree. Branch arms survive if they (or their group) contain a header, which prevents entire conditionals from collapsing simply because one arm lacks annotations.
2. **`hoist_branch_arms`** — recalculates fan-out/fan-in edges for `BranchGroup` and `BranchArm` nodes so every arm receives the parent’s outgoing successors. This is where branch structures become an actual diamond in the graph.
3. **`inline_branch_arms_and_scopes`** — repeatedly inlines any `BranchArm`/`OtherScope` node that still has children by re-parenting those children to the nearest non-branch ancestor, redirecting incoming edges to the first child, and fanning out the node’s successors to every exit in the subtree. This surfaces the enclosed headers at their logical depth without requiring a header to be the first descendant.

All three passes operate on `IndexMap<NodeId, Node>` + `IndexMap<NodeId, Vec<Edge>>`, so converting between raw and flattened representations is just `From` impls.

## Mermaid Rendering
- `mermaid::to_mermaid(&viz)` assigns short aliases (`n0`, `n1`, …), groups children under Mermaid `subgraph` blocks (so lexical hierarchy is visible), and prints edges relative to either the enclosing parent or the root.
- Labels are sanitized via `escape_label` (drops quotes, escapes backslashes/newlines) to avoid breaking Mermaid.
- Styling logic lives on the UI side; here we only expose `describe_node_type` for snapshot readability.

## Tests & Snapshots
- `engine/baml-runtime/src/control_flow/tests.rs::test_snapshots` iterates every `.baml` file under `control_flow/testdata/`. For each target function it records:
  - Human-readable HIR dump of the body (`hir`).
  - Raw graph snapshot (`expr` + Mermaid).
  - Each flattening phase output (label + graph + Mermaid).
- Snapshots live under `engine/baml-runtime/src/control_flow/snapshots/baml_runtime__control_flow__tests__headers__*.snap`.
- Refresh snapshots with:

  ```sh
  INSTA_FORCE_PASS=1 cargo test -p baml-runtime control_flow::tests
  ```

## Practical Notes / Gotchas
- `lexical_id`s are the only persistent identifiers. If you add new `PathSegment` variants or change slug rules, update both graph builder and any consumers that parse these strings.
- Branch groups intentionally lack direct edges until pass 2; if you change `register_child_with_parent`, ensure `hoist_branch_arms` still receives the information it needs.
- `Visit_expression` being intentionally narrow means anything that should produce a visible node must be implemented as either a header, branch, loop, or wrapped block scope. Forgetting to wrap a new construct will silently drop it from the visualization.
- The flattening pipeline assumes header nodes kick off each interesting subtree. If you add new node types that act like headers, you likely need to update `remove_implicit_nodes` and `flatten_branch_arms_and_scopes`.
- Snapshot diffs are the quickest way to validate changes; the Mermaid sandbox is useful when the textual diff is too dense.

This should provide enough detail for a follow-up agent to audit or extend the current control-flow visualization without rediscovering how the traversal and flattening pipeline fit together.
