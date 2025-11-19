# Goal 6 — Control-Flow Graph Structures for Flattening

This note captures the helper routines each flattening pass currently relies on and sketches possible intermediate graph representations that could reduce redundant book-keeping between passes.

## Helpers Per Flattening Pass

### Pass 1 — `remove_implicit_nodes`
- `build_children_map` (from `flatten/mod.rs`) is rebuilt so the pass can walk lexical subtrees.
- `compute_has_header` memoizes whether a node or any descendant contains a header node.
- `should_keep` decides whether the node survives, using the memoized header truth table.
- `filter_viz` rebuilds the `ControlFlowVisualization` with only the retained nodes and edges.

### Pass 2 — `hoist_branch_arms`
- `build_children_map` is constructed again to find branch-arm children.
- `node_depth` walks parent pointers to sort branch groups from deepest to shallowest.
- A bespoke `BranchGroupInfo` struct caches each group’s parent, children, depth, and successors before mutations begin.
- The pass rebuilds per-node edge lists twice: once to drop group → successor edges and again to emit group → arm and arm → successor edges.

### Pass 3 — `inline_branch_arms_and_scopes`
- `build_children_map` is regenerated at the start of every loop iteration so candidate sets stay fresh.
- `collect_candidates` + `dfs_candidates` perform a depth-first walk to order branch-arm/other-scope nodes that still have children.
- `inline_node` performs the actual surgery by calling helper routines:
  - `reparent_children` mutates `parent_node_id`.
  - `redirect_incoming_edges` walks *all* edge lists to retarget predecessors.
  - `collect_exit_nodes`/`collect_exit_nodes_recursive` + `fan_out_outgoing_edges` rediscover every exit before fanning out stored successor edges.

The repeated creation of child maps, depth lookups, and successor scans hints that a richer intermediate representation sitting between passes could amortize this work.

## Option 1 — Precomputed Parent/Child Index
**Description.** Before running any pass, derive a `LexicalIndex` that stores, for every node, its parent, ordered children, depth, and whether a header exists in its subtree. Passes would consume and mutate the index directly instead of rebuilding maps.

**Pros**
- `remove_implicit_nodes` can reuse the subtree header memo without re-traversing.
- `hoist_branch_arms` already needs depth and branch-child queries; both become O(1).
- `inline_branch_arms_and_scopes` can grab current children/exits from the shared index.

**Cons**
- Requires carefully keeping the index in sync with structural edits (re-parenting, deletions).
- Adds memory overhead proportional to node count (extra Vecs/Hashes).
- Mutability story is trickier: passes must update both the index and the base visualization, increasing code complexity.

## Option 2 — Bidirectional Edge Table
**Description.** Convert the graph into a structure that stores both outgoing and incoming edges (`HashMap<NodeId, {out: Vec<Edge>, in: Vec<NodeId>}>`) plus adjacency lists keyed by node type. Each pass receives this richer `BidirectionalGraph` instead of plain `edges_by_src`.

**Pros**
- `inline_branch_arms_and_scopes` no longer needs the expensive `redirect_incoming_edges` scan; incoming edges are already grouped.
- `hoist_branch_arms` can rewire successors by manipulating a single structure, reducing temporary allocations.
- Makes it easier to detect exit nodes (just check empty `out` list) without recomputing child maps.

**Cons**
- Still needs a separate child map for lexical parents unless folded together with Option 1.
- Doubles stored edge data (edges tracked in both directions) unless deduplicated carefully.
- Migration cost: every consumer of `ControlFlowVisualization` that expects only outgoing edges would need adapters or additional conversion.

## Option 3 — Stage-Specific Arena
**Description.** After building the raw visualization, convert it to a mutable arena type (e.g., `FlattenArena`) that stores nodes in a dense `Vec<NodeRecord>` with stable indices and auxiliary per-node metadata slots. Passes operate on the arena, mutating metadata in place and only materialize a standard `ControlFlowVisualization` at the very end.

**Pros**
- Shared per-node metadata (depth, header flags, successor cache) can be placed in arrays parallel to the arena, eliminating repeated hash-map allocations.
- Structural edits (re-parenting, deletion) become localized updates on arena records and side tables, making it cheaper to maintain invariants across passes.
- Enables future passes to annotate nodes (e.g., source spans, UI hints) without touching the public visualization type.

**Cons**
- Larger refactor: every flattening pass must be rewritten against the new arena API.
- Requires conversion adapters at the start/end of the pipeline, adding code paths that must stay in sync.
- Borrowing/mutability rules are more subtle in Rust; naïvely holding multiple mutable references into the arena is unsafe and will need index-based operations or interior mutability.

These options are not mutually exclusive. For example, an arena could include the bidirectional edge table, or the parent/child index could become a view into the arena. The key observation is that investing in a richer intermediate graph should eliminate the repetitive recomputation each pass currently performs.
