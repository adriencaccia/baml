# ControlFlowVisualization Output Specification

This document refines the **User Requirements** from `goals1.md` into an explicit specification for how the control-flow visualization must be produced. Another coding agent should treat the points below as the contract that the implementation in `engine/baml-runtime/src/control_flow.rs` (and its callers) needs to satisfy.

## 1. Purpose
- Emit a `ControlFlowVisualization` for any BAML function such that the UI layer (React mermaid sandbox + future ReactFlow integration) can render a stable, navigable control-flow graph.
- Keep the visualization resilient to minor edits in source (statement insertions, added headers) so previously captured runtime logs continue to correlate with graph nodes.

## 2. Core Data Structures (Rust)
These types live in `engine/baml-runtime/src/control_flow.rs` and must remain the canonical output shape:

```rust
pub struct ControlFlowVisualization {
    pub nodes: HashMap<NodeId, Node>,
    pub edges_by_src: HashMap<NodeId, Vec<Edge>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeId {
    pub function: String,
    pub segments: Vec<PathSegment>,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub id: NodeId,
    pub parent_node_id: Option<NodeId>,
    pub label: String,
    pub span: Span,
    pub node_type: NodeType,
}

#[derive(Clone, Debug)]
pub enum NodeType {
    FunctionRoot,
    ExprBlock,
    Branch,
    Loop,
    Llm { client: String },
    ImpliedByNewScope,
    ImpliedByStatement,
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub src: NodeId,
    pub dst: NodeId,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PathSegment {
    Statement { ordinal: u32 },
    Header { slug: String, ordinal: u16 },
    Scope { kind: ScopeKind, ordinal: u16 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScopeKind {
    FunctionRoot,
    Block,
    LoopBody,
    BranchArm,
}
```

**Specification points:**
- All nodes and edges in the visualization must be addressable by `NodeId`.
- `NodeId::encode()` and `NodeId::from_str()` must remain deterministic inverses for any emitted ID.
- `parent_node_id` should mirror structural scope ancestry (function root has `None`; headers inherit from nearest enclosing scope or header; statements inherit from the scope path).
- `span` must always be populated (fallback to `Span::fake()` when no real span is available).

## 3. NodeId Construction Rules
- Paths are **path-based identifiers**. A node ID equals: `function_name` + ordered `PathSegment`s.
- Segment semantics:
  - `Statement { ordinal }`: `ordinal` is the statement index within the containing block as tracked by a `ScopeFrame`. Multiple nodes derived from the same statement (e.g., implicit scope and actual statement) can reuse the same ordinal but must differ by additional segments (headers/scopes beneath them).
  - `Header { slug, ordinal }`: `slug` is the hyphenated version of the header text (see `slugify` helper), `ordinal` disambiguates duplicate header strings in the same context.
  - `Scope { kind, ordinal }`: push when entering a new lexical scope; `ordinal` increments per nested scope in the same parent so IDs are stable across reordering of different scope kinds.
- The agent must ensure the traversal keeps the cursor and scope stack in sync—every `push_scope` call must eventually be paired with `pop_scope`, and statement/header pushes must be popped immediately after the node has been emitted.

## 4. Node Semantics
- **FunctionRoot**: one per function. Label should be the function name.
- **ExprBlock**: generic expression nodes (includes header nodes and expressions without explicit statements). These represent evaluable expressions in the control flow.
- **Branch**: the control point for an `if` expression or statement; should precede branch arms.
- **Loop**: entry node for any loop flavor. Must emit back-edges labelled `"repeat"` (or flavor-specific label if future changes require).
- **Llm**: for LLM functions only; label must include the client identifier.
- **ImpliedByNewScope**: emitted when entering a scope (block) without an explicit header node.
- **ImpliedByStatement**: emitted for statements without headers (let, assign, etc.) so control flow maintains connectivity.

## 5. Edge Requirements
- `edges_by_src` must, for every source `NodeId`, list **all** outgoing edges.
- Sequential execution: when a new node is emitted, add edges from every node in the current flow frontier to the new node with an empty label (`""`).
- Branches: edges from the branch node to the first node of each arm. After each arm terminates, merge the resulting frontier so downstream nodes receive edges from all exits.
- Loops: edges from loop header to first body node; when finishing the body, add labelled edges from each body exit node back to the loop header; final frontier after loop traversal should contain the loop header to model fallthrough.
- Terminals (`return`, `break`, `continue`, `throw`): after emitting the node, clear the frontier so no subsequent edges originate from those nodes.

## 6. Header Handling
- Every `//#`/`//##`/`//###` header must produce a dedicated node in the graph even if the statement body is empty.
- Slug generation uses ASCII lowercasing and replaces non-alphanumeric characters with a single hyphen; leading/trailing hyphens removed.
- Header nodes should chain: when multiple headers precede the same statement, each header node’s outgoing edge must point to the next header or the statement node.

## 7. Runtime Correlation Guarantees
- IDs must be stable between minor source edits so recorded `TraceContextNotification` events can still be mapped back to nodes.
- Visualization should remain usable even if runtime log order differs from current source (i.e., missing nodes must not crash the consumer; stale IDs may map to slightly different statements but structure should remain consistent).

## 8. Output Validation Requirements
- After traversal `ControlFlowVizBuilder::finish()` must:
  - Ensure every node inserted has a unique `NodeId` key.
  - Group edges by `src` without dropping duplicates.
  - Produce data that round-trips through serialization and `NodeId::from_str()` without panic.
- Snapshot tests (`engine/baml-runtime/src/control_flow/tests.rs::test_snapshots`) should continue to serialize:
  - `expr` map (nodes with spans, parent IDs, labels).
  - `edges` adjacency lists.
  - Mermaid diagram text.

## 9. Stretch Considerations (Informative)
These are not strict requirements but helpful for any fixes:
- When deriving spans, prefer concrete spans from HIR; use fallbacks only when necessary.
- Maintain consistent naming conventions for loop edge labels; if changes are required, propagate them to mermaid serialization and snapshots.

---
**Deliverable:** Updated traversal/serialization code that satisfies the sections above, verified by regenerating snapshots with `INSTA_FORCE_PASS=1 cargo test -p baml-runtime control_flow::tests`.
