# Goal 5d – Elide Empty BranchArm Nodes

## Summary
Add a fourth flattening step that removes `BranchArm` nodes from a `ControlFlowVisualization` after step3. The new pass should splice each `BranchArm` out of the graph by wiring its predecessors directly to its successors, deduplicating edges in the process, while leaving nodes with no predecessors or no successors intact.

## Requirements
1. **Pipeline placement**: invoke the new pass (Step 4) after the current step3 flattening pass. It must operate on the same `ControlFlowVisualization` structure produced by earlier steps.
2. **Node removal**: for each `BranchArm` node, inspect its incoming and outgoing edges.
   - If it has at least one predecessor and at least one successor, remove the node from `nodes` and delete all edges touching it. Insert new edges from every predecessor directly to every successor.
   - If it has zero predecessors *or* zero successors, leave the node untouched.
3. **Edge deduplication**: ensure the resulting edge set contains no duplicate `src → dst` pairs after rewiring. Keep ordering stable if possible.
4. **Data integrity**: once a `BranchArm` is removed, make sure it no longer appears anywhere in the graph (nodes map, edge lists, or auxiliary indices) and that other node/edge metadata remains unchanged.
5. **Testing**: update/run the relevant `control_flow` flattening snapshot tests so they capture the new structure without `BranchArm` nodes.

## Notes
- Treat each `BranchArm` independently; there is no need to special-case branch groups or other node types.
- Preserve spans, lexical IDs, and edge ordering for all surviving nodes to avoid churn in downstream tools.
