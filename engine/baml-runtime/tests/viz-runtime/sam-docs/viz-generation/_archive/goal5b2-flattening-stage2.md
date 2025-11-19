# Goal 5b2 – Hoist Branch Arms (Step 2) Fix Plan

This note explains what is missing in `engine/baml-runtime/src/control_flow/flatten/step2_hoist_branch_arms.rs` and how to update it so another coding agent can implement the change confidently.

## Current Behavior (Pass 2: `hoist_branch_arms`)
- The pass clones the incoming `ControlFlowVisualization`, finds `NodeType::BranchGroup` nodes, and:
  1. Replaces the branch group’s outgoing edges so they point to each `BranchArm` child.
  2. Copies the branch group’s original successors onto every branch arm (deduplicated).
- **What it does _not_ do:** adjust the tree structure. Branch groups remain parents of branch arms, and branch arms remain nested children rather than peers of subsequent nodes.
- The existing test (`branch_group_viz`) only asserts that new edges were added; it never verifies node re-parenting or that branch arms now participate in the linear order of their grandparent.

## Desired Behavior
The Hoist Branch Arms stage should fully “inline” branch groups / branch arms so the tree looks like:

```
Before:
A → BranchGroupB → C
        ↘ BranchArm1
        ↘ BranchArm2
        ↘ BranchArm3

After:
A → BranchGroupB → {BranchArm1, BranchArm2, BranchArm3}
BranchArm* → C (all share the original successor set)
```

In other words, for every `BranchGroup` node, we should apply the following changes (working through `BranchGroup` nodes in DFS order, deepest first):

1. The incoming edges to the `BranchGroup` stay exactly as they were. Predecessor nodes still point to the `BranchGroup` node, and the `BranchGroup` node itself keeps its parent.
2. For every outgoing edge from the `BranchGroup` node:
	1. we should establish an edge from each `BranchArm` child to the successor of the outgoing edge
	2. and then drop the edge from the `BranchGroup` node
3. Then each `BranchArm` child should be hoisted up a level:
	1. change the `BranchArm`'s parent from the `BranchGroup` to the `BranchGroup`'s parent
	2. create an edge from the `BranchGroup` to the `BranchArm`

## Implementation Plan
1. **Collect metadata in depth-first order (deepest-first):**
   - Record each `BranchGroup`’s depth, parent ID, child `BranchArm`s, and the original successor set.
   - Sort the branch groups by depth descending so we mutate children before their ancestors, matching the desired behavior section.
2. **Rewrite edges for each branch group:**
   - Replace the branch group’s `edges_by_src` entry with edges to each branch arm (if there are no branch arms, skip the rewrite).
   - For each original outgoing edge `(BranchGroup → successor)`, add edges `(BranchArm → successor)` for every child arm, deduplicating per arm.
   - Drop the original `BranchGroup → successor` edges once the arms inherit them.
3. **Hoist branch arms (group stays put):**
   - Set each branch arm’s `parent_node_id` to the parent of its branch group so the arm becomes a sibling in the surrounding scope.
   - Because the branch group remains in place, the incoming edges are unchanged; only its children move.
4. No additional `build_children_map` fiddling is necessary; it will naturally reflect the new parents the next time it is rebuilt.

## Testing Guidance
1. Extend `branch_group_viz` (or add a new scenario) so it asserts:
   - `expanded.nodes[&BranchArm].parent_node_id == Some(root.id)` (i.e., the same parent as the branch group’s parent).
   - The siblings order remains stable (controller uses `IndexMap`, so insertion order matters).
2. Ensure the test validates the new edge shape exactly:
   ```rust
   assert_eq!(
       expanded
           .edges_by_src
           .get(&group_id)
           .unwrap()
           .iter()
           .map(|e| e.dst.raw())
           .collect::<Vec<_>>(),
       vec![arm1_id.raw(), arm2_id.raw(), arm3_id.raw()],
   );
   assert_eq!(
       expanded
           .edges_by_src
           .get(&arm1_id)
           .unwrap()
           .iter()
           .map(|e| e.dst.raw())
           .collect::<Vec<_>>(),
       vec![after_id.raw()],
   );
   // ...repeat for other arms.
   ```
   Also assert that the branch group no longer has direct edges to the successor(s)—only to its branch arms.
3. Consider adding a regression test where a branch group had no successors; branch arms should then have zero outgoing edges as well (no panic).

## Notes / Risks
- Stage 3 (`flatten_branch_arms_and_scopes`) assumes branch arms remain children of branch groups until it runs. After we change Stage 2, reevaluate that pass to ensure it still detects header-carrying arms correctly. It may need to look at previous parent IDs or traverse the tree differently.
- Updating the pass will change snapshot outputs (Mermaid diagrams, JSON). Be ready to rerun `INSTA_UPDATE=always cargo test -p baml-runtime control_flow::tests` and inspect sandbox results.
- Keep `IndexMap` ordering stable: when you re-parent nodes, avoid removing/reinserting them so node iteration order (which influences snapshot determinism) doesn’t shuffle unexpectedly.
