# Goal
Implement step 3 of the flattening pipeline (`engine/baml-runtime/src/control_flow/flatten/step3_inline_implicit_nodes.rs`) by inlining BranchArm and OtherScope nodes.

## Desired Behavior
The running example below starts with the step-2 output and shows the step-3 output we should implement.

```
# step 2 output
Nodes:
    - n0: parent=None, kind=FunctionRoot("BranchArmsContainSequentialHeaders")
        - n1: parent=n0, kind=HeaderContextEnter("Enclosing")
            - n2: parent=n1, kind=BranchGroup("if (k == 1)")
            - n3: parent=n1, kind=BranchArm("if (k == 1)")
                - n4: parent=n3, kind=HeaderContextEnter("k == 1 BranchArm first")
                - n5: parent=n3, kind=HeaderContextEnter("k == 1 BranchArm second")
            - n6: parent=n1, kind=BranchArm("else if (k == 2)")
                - n7: parent=n6, kind=HeaderContextEnter("k == 2 BranchArm first")
                - n8: parent=n6, kind=HeaderContextEnter("k == 2 BranchArm second")
                - n9: parent=n6, kind=HeaderContextEnter("k == 2 BranchArm third")
            - n10: parent=n1, kind=BranchArm("else if (k == 3)")
            - n11: parent=n1, kind=BranchArm("else")
                - n12: parent=n11, kind=HeaderContextEnter("Else Branch")
            - n13: parent=n1, kind=OtherScope
                - n14: parent=n13, kind=HeaderContextEnter("After")

edges_by_src:
    n2: [n3, n6, n10, n11]
    n4: [n5]
    n7: [n8]
    n8: [n9]
    n3: [n13]
    n6: [n13]
    n10: [n13]
    n11: [n13]
```

```
# step 3 output
Nodes:
    - n0: parent=None, kind=FunctionRoot("BranchArmsContainSequentialHeaders")
        - n1: parent=n0, kind=HeaderContextEnter("Enclosing")
            - n2: parent=n1, kind=BranchGroup("if (k == 1)")
            - n4: parent=n1, kind=HeaderContextEnter("k == 1 BranchArm first")
            - n5: parent=n1, kind=HeaderContextEnter("k == 1 BranchArm second")
              
            - n7: parent=n1, kind=HeaderContextEnter("k == 2 BranchArm first")
            - n8: parent=n1, kind=HeaderContextEnter("k == 2 BranchArm second")
            - n9: parent=n1, kind=HeaderContextEnter("k == 2 BranchArm third")
              
            - n10: parent=n1, kind=BranchArm("else if (k == 3)") // kept because it had no children
            - n12: parent=n1, kind=HeaderContextEnter("Else Branch")
              
            - n14: parent=n1, kind=HeaderContextEnter("After") // inlined from OtherScope

edges_by_src:
    n2: [n4, n7, n10, n12]     // BranchGroup now points directly to the first node in every BranchArm
    n4: [n5]
    n5: [n14]                  // outgoing edges from the BranchArm are applied to each exit node in the BranchArm
    n7: [n8]
    n8: [n9]
    n9: [n14]
    n10: [n14]
    n12: [n14]
```


### BranchArm rules
1. A `BranchArm` is inlined if it has any children.
2. When inlining:
   - Re-parent **all direct children** of the arm (headers, loops, nested branch groups, etc.) to the arm’s parent (already the surrounding scope thanks to step 2). This keeps the original insertion order because `IndexMap` retains order without reinsertion.
   - Remove the `BranchArm` node from `viz.nodes`.
   - Every incoming control-flow edge that previously targeted the arm must now point to the arm’s **entry header** (the earliest direct `HeaderContextEnter` child).
   - Every outgoing edge that previously left the arm must now leave from each **exit node** in the arm’s subtree (the nodes whose outgoing edges are either empty or confined within the subtree). That keeps fall-through edges intact even when multiple headers exist inside the branch.

### OtherScope rules
`OtherScope` nodes are inlined the same way:
- Require at least one direct child; otherwise leave the scope alone.
- Hoist their direct children to the scope’s parent, delete the `OtherScope`, redirect incoming edges to the entry header, and redirect the deleted scope’s outgoing edges to every exit node.

## Implementation Details
Implement step 3 as a single in-place graph transform inside `Step3InlineImplicitNodes::run`. The routine should iterate through `viz.nodes` in insertion order, detect `BranchArm` or `OtherScope` candidates with at least one direct child, and enqueue them for processing; don’t mutate the map while iterating because `IndexMap` indices invalidate. After the scan, process each candidate by:
1. **Computing structure:** Gather the parent node ID (already stored on the node), the list of direct children (`viz.nodes.values().filter(|child| child.parent_node_id == Some(candidate_id))`), and the candidate’s incoming/outgoing edges by consulting `viz.edges_by_src` plus a reverse lookup built on demand (e.g., `HashMap<NodeId, Vec<NodeId>>` derived once at the start).
2. **Determining entry and exit nodes:** The entry node is the earliest child that matches `HeaderContextEnter` or another structural node; because children retain insertion order, the first element suffices. Exit nodes are computed via a DFS limited to the candidate’s subtree: a node is an exit if it has no outgoing edges or only edges that stay in the subtree. Cache the subtree membership in a `HashSet<NodeId>` for O(1) containment tests.
3. **Hoisting children:** Update each direct child’s `parent_node_id` to the candidate’s parent. No reordering is needed—`IndexMap` maintains the original sequence, so after the candidate is removed the hoisted children naturally occupy the correct slot.
4. **Rewriting edges:**  
   - For every predecessor recorded in the reverse lookup, replace edges targeting the candidate with edges targeting the entry node (preserve multiplicity).  
   - Remove the candidate’s outbound edge list from `edges_by_src` and clone it into a temporary vector. For every exit node, append the cloned targets to `edges_by_src[exit_id]`, inserting the entry if it did not previously exist.
5. **Deleting the node:** Remove the candidate from `viz.nodes`. There’s no need to compact node IDs; consumers rely on lexical IDs for stability.

When all candidates are processed, the resulting graph should mirror the “step 3 output” example. Add regression coverage by extending `control_flow::tests::flatten::step3` snapshots to include cases with mixed branch arms, nested scopes, and fall-through edges.
