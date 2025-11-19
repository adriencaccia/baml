status: DONE
# ControlFlowVisualization Fix Plan

This document describes the concrete fixes required in `engine/baml-runtime/src/control_flow.rs` (and supporting modules) so the compiler produces `ControlFlowVisualization` instances that match `goal4b-visualization-structure.md`.

## Overview of Required Fixes
1. **Node Taxonomy Alignment** – update the node/edge data model to expose the exact variants and labels used in the spec.
2. **Header Hierarchy Construction** – drive parent/child relationships for `//#`, `//##`, `//###`, … headers from a shared traversal stack.
3. **Branch and Loop Modeling** – emit `BranchGroup`, `BranchArm`, and `Loop` nodes with the parentage shown in the examples while avoiding synthetic control-flow edges.
4. **OtherScope Coverage** – represent implicit scopes (blocks, expression bodies) using `OtherScope` nodes and attach nested headers correctly.
5. **Sibling-Only Edges** – restrict `edges_by_src` to consecutive header nodes that share the same parent; omit branch/loop/return edges entirely.

The sections below expand each fix and point to the implementation work needed.

---

## 1. Node Taxonomy Alignment
**Bug**: The current runtime still exposes `NodeType::ImpliedByNewScope`, `NodeType::ImpliedByStatement`, etc., which do not exist in `goal4b`. That mismatch prevents the visualization from matching the documented structure.

**Required behavior**:
- `NodeType` must contain exactly these variants: `FunctionRoot`, `HeaderContextEnter`, `BranchGroup`, `BranchArm`, `Loop`, and `OtherScope`.
- `Node.label` carries the human-readable string (header text, branch condition, `else`, loop header, scope description).
- `Edge` no longer needs labels; the spec treats each edge as unlabeled.

**Implementation guidance**:
- Replace the existing `NodeType` enum with the six variants above and adjust constructors/call sites accordingly.
- Rename helper constructors (`Node::branch`, `Node::loop`, etc.) or add new ones so each variant is created with the correct label and span.
- Remove any references to deprecated variants (`ImpliedBy*`, `ExprBlock`, `Branch`) and migrate their responsibilities to `HeaderContextEnter`, `BranchGroup`, `BranchArm`, `Loop`, and `OtherScope`.
- Simplify `Edge` to `(src, dst)` only; drop label plumbing if nothing else needs it.
- Update serializers, tests, and snapshots to account for the new variants and any formatting differences.

---

## 2. Header Hierarchy Construction
**Bug**: Header nodes are currently chained linearly, ignoring their nesting level. This contradicts the nested tree shown in Examples 1, 5, 12, and 13.

**Required behavior**:
- A header’s level is determined by the number of `#` characters (`//#` → level 1, `//##` → level 2, etc.).
- The parent of a new header is:
  - The most recent header with a strictly smaller level inside the same lexical scope; or
  - The lexical scope node itself (function root, block `OtherScope`, branch arm, loop) when no shallower header is active.
- Headers remain active until a sibling at the same (or shallower) level is encountered or the traversal exits the lexical scope.

**Implementation guidance**:
- Maintain a single traversal stack that interleaves lexical scopes and active headers:

  ```rust
  enum Frame {
      Scope { kind: ScopeKind, node_id: NodeId },
      Header { level: u8, node_id: NodeId },
  }
  ```

- When you see a header comment:
  1. Determine its level.
  2. Pop `Frame::Header` entries whose level is ≥ the new level (only within the current scope).
  3. Use the node ID on the new stack top (another header or a scope) as the parent for the new header node.
  4. Push the new `Header` frame so nested headers and statements can attach beneath it.
- On lexical scope exit (see Fix #4), pop frames back to the matching `Frame::Scope` so headers introduced inside the scope cannot leak outward.
- Rebuild `emit_header_nodes`/equivalent helpers around this unified stack. Avoid duplicating state in separate “header stacks”.

**Regression checks**:
- Add/update fixtures that mix `//#`, `//##`, `//###` headers and verify the expected parent chains in test snapshots.

---

## 3. Branch and Loop Modeling
**Bug**: The current traversal emits nodes like `ImpliedByNewScope` or attaches branch bodies under generic nodes. `goal4b` instead requires explicit `BranchGroup` and `BranchArm` nodes, with loop nodes acting as parents for their bodies, and no synthetic edges inside the branch/loop.

**Required behavior**:
- For every conditional expression:
  - Emit a `BranchGroup` node labeled with the condition (`if (…)`, `match (…)`, etc.) under the header or scope that introduced it.
  - For each arm, emit a `BranchArm` node labeled with the arm discriminator (`if`, `else if`, `else`, `case`, etc.) as a child of the `BranchGroup`.
  - Statements within the arm (headers, nested scopes) become descendants of the arm node.
- For loops:
  - Emit a `Loop` node labeled with the loop source (`for (…)`, `while (…)`, etc.).
  - Headers inside the loop attach beneath the `Loop` node.
  - Do **not** add edges from the loop body back to the loop node; repetition is implied by the parent-child relationship.
- Early `return`, `break`, and `continue` statements are intentionally not modeled in `edges_by_src` (Example 10).

**Implementation guidance**:
- Update branch-walking helpers to build the two-level structure (group + arm) and to reuse the traversal stack so headers/nested scopes see the arm as their immediate parent.
- Ensure the loop visitor always pushes a `Frame::Scope` for the `Loop` node so nested headers become children of the loop.
- Remove any code that attempts to wire control-flow edges between branch arms, loop exits, or next statements.
- Extend or add unit tests covering `if/else`, `else if` chains, nested conditionals, and loops to confirm the resulting node hierarchy matches the spec.

---

## 4. OtherScope Coverage
**Bug**: Blocks introduced by braces or expression bodies either reuse the wrong parent or are represented by legacy `ImpliedBy*` nodes.

**Required behavior**:
- Whenever the traversal encounters a scope that is not a function root, branch group, branch arm, or loop (e.g., `{ … }` blocks, `let config = { … }`, inline closures), emit an `OtherScope` node.
- The `OtherScope` label should describe the scope when we have that information (Example 9) or remain empty/derived from syntax when we do not (Example 7).
- Headers and nested scopes declared inside the block attach beneath the `OtherScope` node and can add edges among themselves (Example 9: `Populate defaults` → `Apply overrides`).

**Implementation guidance**:
- When entering one of these scopes, emit the `OtherScope` node, push `Frame::Scope` for it, and ensure any headers or nested scopes use its node ID as their parent.
- On exit, pop the scope frame along with any headers introduced inside it (handled by the shared stack).
- Replace legacy `ImpliedByNewScope` semantics with `OtherScope`.

**Regression checks**:
- Add tests for blocks both with and without header comments to confirm they attach beneath the correct parent.

---

## 5. Sibling-Only Edges
**Bug**: The existing builder connects edges across scope boundaries and occasionally keeps loop/branch edges alive. `goal4b` shows edges only between adjacent headers that share the same parent (Examples 1, 2, 5, 9, 12).

**Required behavior**:
- Maintain separate sequencing within each parent node:
  - When a new header node `Hn` is emitted, connect the previous header under the same parent to `Hn`.
  - No edges should be emitted for branch groups, branch arms, loops, or other scope nodes.
  - Do not create edges that jump between parents (e.g., from a header under `BranchArm` to a header under the function root).
- Omit edges that would represent early returns, breaks, or continues. Execution that stops early is left implicit.

**Implementation guidance**:
- Store the “last header child” on the active scope/header frame itself (e.g., `Frame::Scope { …, last_header_child: Option<NodeId> }`) so no separate map is needed.
- When emitting a `HeaderContextEnter` node, look at the topmost scope/header frame with a `last_header_child`; if it matches the same parent, emit the edge and update the stored node ID.
- Reset the stored `last_header_child` when you pop the frame so siblings higher in the tree retain their own sequencing.
- Update snapshots to reflect the reduced edge set (e.g., Example 5 shows `n2 -> n4` because both headers share parent `n1`).

---

## Deliverables & Validation Steps
1. Update the runtime implementation to satisfy Sections 1–5 above.
2. Expand unit tests/fixtures to cover the scenarios in `goal4b-visualization-structure.md` (multiple header depths, branch/loop combinations, other scopes, early returns).
3. Regenerate any insta snapshots:
   ```sh
   INSTA_FORCE_PASS=1 cargo test -p baml-runtime control_flow::tests
   ```
4. Verify the emitted `ControlFlowVisualization` for each fixture matches the hierarchy and edge sets illustrated in `goal4b`.
5. Record any remaining gaps (if any) for future follow-up; do not reintroduce legacy node types or cross-scope edges.

---

## Edge Cases to Double-Check
- Headers with no body before the next sibling or scope end: ensure they are popped from the stack so later nodes do not inherit them.
- Headers after nested blocks: confirm that leaving a scope pops any headers introduced inside it, allowing subsequent siblings to attach to their correct parents.
- Expression blocks without header comments: they should still produce an `OtherScope` node, but no edges.
- Nested conditionals and loops: verify that branch arms and loop bodies never receive synthetic edges yet their header children remain connected via `edges_by_src`.
- Early `return`, `break`, `continue`: ensure there is no edge skipping past the terminator; this limitation is explicitly acknowledged in the spec.

Following this plan will bring the traversal logic in line with `goal4b-visualization-structure.md` and provide deterministic, structured control-flow visualizations for expression functions.
