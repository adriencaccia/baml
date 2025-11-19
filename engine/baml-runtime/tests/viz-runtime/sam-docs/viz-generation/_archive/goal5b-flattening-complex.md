The BAML compiler should be able to, for a given expression function containing `//#` `//##` `//###` HeaderContextEnter statements, build a `ControlFlowVisualization` for that function with the below structure, and that flatten the resulting ControlFlowVisualization into a FlattenedControlFlowVisualization which will be more readable to the user. 
```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeId {
    function: String,
    segments: Vec<PathSegment>,
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
    HeaderContextEnter,
    BranchGroup,
    BranchArm,
    Loop,
    OtherScope,
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub src: NodeId,
    pub dst: NodeId,
}

#[derive(Clone, Debug, Default)]
pub struct ControlFlowVisualization {
    pub nodes: HashMap<NodeId, Node>,
    pub edges_by_src: HashMap<NodeId, Vec<Edge>>,
}


#[derive(Clone, Debug, Default)]
pub struct FlattenedControlFlowVisualization {
    pub nodes: HashMap<NodeId, Node>,
    pub edges_by_src: HashMap<NodeId, Vec<Edge>>,
}
```

## Algorithm
Flattening prioritizes these user requirements:

- HeaderContextEnter nodes will always be shown
- implied nodes (BranchGroup, BranchArm, Loop, OtherScope) should only be shown as necessary for a given HeaderContextEnter to make sense

To achieve these goals, the algorithm will make three passes over the graph, applying the following changes:
1. remove implicit nodes
	- within a BranchGroup, every BranchArm sibling should be shown if any contains a HeaderContextEnter node
	- otherwise, we should preserve only nodes that contain a HeaderContextEnter in their subtree
	- e.g. if a BranchGroup contains only leaf BranchArm nodes, that BranchGroup and its children should be removed
2. BranchGroup and BranchArm nodes should be flattened, and have new edges created
	```
	nodes:
	- A
	- BranchGroupB
		- BranchArm1
		- BranchArm2
		- BranchArm3
	- C
	  
	edges_by_src:
	- A: [BranchGroupB]
	- BranchGroupB: [C]
	```

	should get flattened into

	```
	nodes:
	- A
	- BranchGroupB
	- BranchArm1
	- BranchArm2
	- BranchArm3
	- C
	  
	edges_by_src:
	- A: [BranchGroupB]
	- BranchGroupB: [BranchArm1, BranchArm2, BranchArm3]
	- BranchArm1: [C]
	- BranchArm2: [C]
	- BranchArm3: [C]
	```

3. BranchArm and OtherScope nodes should be flattened out, so

	```
	nodes:
	- A
	- BranchGroupB
		- BranchArm1
			- Header1
			- Header2
		- BranchArm2
		- BranchArm3
	- C
	  
	edges_by_src:
	- A: [BranchGroupB]
	- BranchGroupB: [C]
	```

	should get flattened into

	```
	nodes:
	- A
	- BranchGroupB
	- Header1
	- Header2
	- BranchArm2
	- BranchArm3
	- C
	  
	edges_by_src:
	- A: [BranchGroupB]
	- BranchGroupB: [Header1, BranchArm2, BranchArm3]
	- Header1: [Header2]
	- Header2: [C]
	- BranchArm1: [C]
	- BranchArm2: [C]
	- BranchArm3: [C]
	```

# Examples
Here are examples of how this logic works:

## Example 1
```baml
function Foo() -> Void {
  //# Do lorem things
  Lorem();
  
  if (true) {
    asdf();
  } else {
    jklmnop();
  }
  
  //# Prepare ipsum recipes
  Ipsum();
}
```

will result in this ControlFlowVisualization:
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("Foo")
        - n1: parent=n0, kind=HeaderContextEnter("Do lorem things")
            - n2: parent=n1, kind=BranchGroup("if (true)")
                - n3: parent=n2, kind=BranchArm("if (true)")
                - n4: parent=n2, kind=BranchArm("else")
        - n5: parent=n0, kind=HeaderContextEnter("Prepare ipsum recipes")

edges_by_src:
    n1: [n5]
```



should get flattened into:

```
Nodes:
    - n0: parent=None, kind=FunctionRoot("Foo")
        - n1: parent=n0, kind=HeaderContextEnter("Do lorem things")
        - n5: parent=n0, kind=HeaderContextEnter("Prepare ipsum recipes")

edges_by_src:
    n1: [n5]
```

## Example 2
```baml
function Foo() -> Void {
  //# Do lorem things
  Lorem();
  
  //# Do lorem things
  if (true) {
    asdf();
  } else {
    let k = 5;
    if (false) {
      //# should not happen
      IfFalse(k)
    } else {
      CountCache(k);
    }
  }
  
  //# Prepare ipsum recipes
  Ipsum();
}
```

will result in this ControlFlowVisualization:
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("Foo")
        - n1: parent=n0, kind=HeaderContextEnter("Do lorem things")
        - n2: parent=n0, kind=HeaderContextEnter("Do lorem things")
            - n3: parent=n2, kind=BranchGroup("if (true)")
                - n4: parent=n3, kind=BranchArm("if (true)")
                - n5: parent=n3, kind=BranchArm("else")
                    - n6: parent=n5, kind=BranchGroup("if (false)")
                        - n7: parent=n6, kind=BranchArm("if (false)")
                            - n8: parent=n7, kind=HeaderContextEnter("should not happen")
                        - n9: parent=n6, kind=BranchArm("else")
        - n10: parent=n0, kind=HeaderContextEnter("Prepare ipsum recipes")

edges_by_src:
    n1: [n2]
    n2: [n10]
```

should get flattened into
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("Foo")
        - n1: parent=n0, kind=HeaderContextEnter("Do lorem things")
        - n2: parent=n0, kind=HeaderContextEnter("Do lorem things")
            - n3: parent=n2, kind=BranchGroup("if (true)")
			- n4: parent=n2, kind=BranchArm("if (true)")
			- n5: parent=n2, kind=BranchArm("else")
				- n6: parent=n5, kind=BranchGroup("if (false)")
				- n7: parent=n5, kind=BranchArm("if (false)")
					- n8: parent=n7, kind=HeaderContextEnter("should not happen")
				- n9: parent=n5, kind=BranchArm("else")
        - n10: parent=n0, kind=HeaderContextEnter("Prepare ipsum recipes")

edges_by_src:
    n1: [n2]
    n2: [n10]
    n3: [n4, n5]
    n6: [n7, n9]
```


## Example 3
```baml
function Foo() -> Void {
  //# Do lorem things
  Lorem();
  
  if (true) {
    asdf();
  } else {
    //# inject placeholder text
    jklmnop();
  }
  
  //# Prepare ipsum recipes
  Ipsum();
}
```

will result in this ControlFlowVisualization:
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("Foo")
        - n1: parent=n0, kind=HeaderContextEnter("Do lorem things")
            - n2: parent=n1, kind=BranchGroup("if (true)")
                - n3: parent=n2, kind=BranchArm("if (true)")
                - n4: parent=n2, kind=BranchArm("else")
                    - n5: parent=n4, kind=HeaderContextEnter("inject placeholder text")
        - n6: parent=n0, kind=HeaderContextEnter("Prepare ipsum recipes")

edges_by_src:
    n1: [n6]
```

should get flattened into:
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("Foo")
        - n1: parent=n0, kind=HeaderContextEnter("Do lorem things")
            - n2: parent=n1, kind=BranchGroup("if (true)")
			- n3: parent=n1, kind=BranchArm("if (true)")
			- n4: parent=n1, kind=BranchArm("else")
				- n5: parent=n4, kind=HeaderContextEnter("inject placeholder text")
        - n6: parent=n0, kind=HeaderContextEnter("Prepare ipsum recipes")

edges_by_src:
    n1: [n6]
    n2: [n3, n4]
```

## Example 4
```baml
function Foo() -> Void {
  //# Do lorem things
  Lorem();
  
  if (true) {
    asdf();
  } else {
    let k = 5;
    if (false) {
      //# should not happen
      IfFalse(k)
    } else {
      CountCache(k);
    }
  }
  
  //## Ipsum
  Ipsum();
  
  //# Prepare dolor recipes
  Dolor();
}
```

will result in this ControlFlowVisualization:
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("Foo")
        - n1: parent=n0, kind=HeaderContextEnter("Do lorem things")
            - n2: parent=n1, kind=BranchGroup("if (true)")
                - n3: parent=n2, kind=BranchArm("if (true)")
                - n4: parent=n2, kind=BranchArm("else")
                    - n5: parent=n4, kind=BranchGroup("if (false)")
                        - n6: parent=n5, kind=BranchArm("if (false)")
                            - n7: parent=n6, kind=HeaderContextEnter("should not happen")
                        - n8: parent=n5, kind=BranchArm("else")
	        - n9: parent=n1, kind=HeaderContextEnter("Ipsum")
        - n10: parent=n0, kind=HeaderContextEnter("Prepare dolor recipes")

edges_by_src:
    n1: [n10]
    n2: [n9]
    n5: [n6, n8]
    n7: [n9]
    n9: [n10]
```

should get flattened into:
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("Foo")
        - n1: parent=n0, kind=HeaderContextEnter("Do lorem things")
            - n2: parent=n1, kind=BranchGroup("if (true)")
            - n3: parent=n1, kind=BranchArm("if (true)")
            - n5: parent=n1, kind=BranchGroup("if (false)")
			- n7: parent=n1, kind=HeaderContextEnter("should not happen")
            - n8: parent=n1, kind=BranchArm("else")
	        - n9: parent=n1, kind=HeaderContextEnter("Ipsum")
        - n10: parent=n0, kind=HeaderContextEnter("Prepare dolor recipes")

edges_by_src:
    n1: [n10]
    n2: [n3, n5]
    n5: [n7, n8]
    n7: [n9]
    n8: [n9]
    n9: [n10]
```

## Example 5
```baml pseudocode
function Foo() -> Void {
  //# Do lorem things
  Lorem();
  
  if (true) {
    //# set up asdf
    asdf();
  }

  //# Prepare ipsum recipes
  Ipsum();
}
```

will result in this ControlFlowVisualization:
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("Foo")
        - n1: parent=n0, kind=HeaderContextEnter("Do lorem things")
            - n2: parent=n1, kind=BranchGroup("if (true)")
                - n3: parent=n2, kind=BranchArm("if (true)")
                    - n4: parent=n3, kind=HeaderContextEnter("set up asdf")
        - n5: parent=n0, kind=HeaderContextEnter("Prepare ipsum recipes")

edges_by_src:
    n1: [n5]
```
should get flattened into:
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("Foo")
        - n1: parent=n0, kind=HeaderContextEnter("Do lorem things")
            - n2: parent=n1, kind=BranchGroup("if (true)")
            - n3: parent=n1, kind=BranchArm("if (true)")
                - n4: parent=n3, kind=HeaderContextEnter("set up asdf")
        - n5: parent=n0, kind=HeaderContextEnter("Prepare ipsum recipes")

edges_by_src:
    n1: [n5]
    n2: [n3]
```

## Example 6
Shows an `if/else` where each BranchGroup acquires its own header context.
```baml pseudocode
function Choose() -> String {
  //# Build primary option
  buildPrimary();

  if (isReady) {
    //# Use primary
    finalizePrimary();
  } else {
    //# Use fallback 
    fallback();
  }

  //# Wrap up choice
  wrap();
  return outcome;
}
```

will result in this ControlFlowVisualization:
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("Choose")
        - n1: parent=n0, kind=HeaderContextEnter("Build primary option")
            - n2: parent=n1, kind=BranchGroup("if (isReady)")
                - n3: parent=n2, kind=BranchArm("if (isReady)")
                    - n4: parent=n3, kind=HeaderContextEnter("Use primary")
                - n5: parent=n2, kind=BranchArm("else")
                    - n6: parent=n5, kind=HeaderContextEnter("Use fallback")
        - n7: parent=n0, kind=HeaderContextEnter("Wrap up choice")

edges_by_src:
    n1: [n7]
```

should get flattened into
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("Choose")
        - n1: parent=n0, kind=HeaderContextEnter("Build primary option")
            - n2: parent=n1, kind=BranchGroup("if (isReady)")
            - n3: parent=n1, kind=BranchArm("if (isReady)")
                - n4: parent=n3, kind=HeaderContextEnter("Use primary")
            - n5: parent=n1, kind=BranchArm("else")
                - n6: parent=n5, kind=HeaderContextEnter("Use fallback")
        - n7: parent=n0, kind=HeaderContextEnter("Wrap up choice")

edges_by_src:
    n1: [n7]
    n2: [n3, n5]

```

## Example 12
Captures an `if / else if / else` chain.
```baml pseudocode
function EvaluateStatus(level: Text) -> Void {
  //# Assess level
  CheckPreconditions();
  
  if (level == "urgent") {
    //# Handle urgent
    escalateToTierOne();
    //# Page escalation ladder
    escalateToManagers();
  } else if (level == "warning") {
    warnTeam();
  } else {
    //# Handle normal
    proceed();
  }
  
  if (level.includes("phone")) {
    pageViaPhoneCalls();
  } else {
    pageViaApp();
  }

  //# Finalize evaluation
  closeOut();
}
```

will result in this ControlFlowVisualization:
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("EvaluateStatus")
        - n1: parent=n0, kind=HeaderContextEnter("Assess level")
            - n2: parent=n1, kind=BranchGroup("if (level == \"urgent\")")
				- n3: parent=n2, kind=BranchArm(""if (level == \"urgent\")")
                    - n4: parent=n3, kind=HeaderContextEnter("Handle urgent")
                    - n5: parent=n3, kind=HeaderContextEnter("Page escalation ladder")
                - n6: parent=n2, kind=BranchArm("else if (level == \"warning\")")
                - n8: parent=n2, kind=BranchArm("else")
                    - n9: parent=n8, kind=HeaderContextEnter("Handle normal")
            - n10: parent=n1, kind=BranchGroup("if (level.includes(\"phone\"))")
                - n11: parent=n10, kind=BranchArm("if (level.includes(\"phone\"))")
                - n12: parent=n10, kind=BranchArm("else")
        - n13: parent=n0, kind=HeaderContextEnter("Finalize evaluation")

edges_by_src:
    n1: [n13]
    n4: [n5]
```

which will be flattened like so:

```
Nodes:
    - n0: parent=None, kind=FunctionRoot("EvaluateStatus")
        - n1: parent=n0, kind=HeaderContextEnter("Assess level")
            - n2: parent=n1, kind=BranchGroup("if (level == \"urgent\")")
			- n4: parent=n1, kind=HeaderContextEnter("Handle urgent")
			- n5: parent=n1, kind=HeaderContextEnter("Page escalation ladder")
            - n6: parent=n1, kind=BranchArm("else if (level == \"warning\")")
			- n9: parent=n1, kind=HeaderContextEnter("Handle normal")
        - n13: parent=n0, kind=HeaderContextEnter("Finalize evaluation")

edges_by_src:
    n1: [n13]
    n2: [n4, n6, n9]
    n4: [n5]
```

## Example 13
Loop nodes do not get flattened.
```baml pseudocode
function DrainQueue(queue: Queue<Item>) -> Void {
  //# Prepare queue read
  prime(queue);

  while (queue.hasNext()) {
    //# Process next entry
    handle(queue.next());
  }

  //# Close queue read
  cleanup(queue);
}
```

will result in this ControlFlowVisualization:
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("DrainQueue")
        - n1: parent=n0, kind=HeaderContextEnter("Prepare queue read")
            - n2: parent=n1, kind=Loop("while (queue.hasNext())")
                - n3: parent=n2, kind=HeaderContextEnter("Process next entry")
        - n4: parent=n0, kind=HeaderContextEnter("Close queue read")

edges_by_src:
    n1: [n4]
    n2: [n3]
```

which will be flattened like so:
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("DrainQueue")
        - n1: parent=n0, kind=HeaderContextEnter("Prepare queue read")
            - n2: parent=n1, kind=Loop("while (queue.hasNext())")
                - n3: parent=n2, kind=HeaderContextEnter("Process next entry")
        - n4: parent=n0, kind=HeaderContextEnter("Close queue read")

edges_by_src:
    n1: [n4]
    n2: [n3]
```

## Example 14
Illustrates a loop whose body contains a multi-arm conditional; because one arm has a header, every sibling BranchArm must also stay visible after flattening.
```baml pseudocode
function FilterRecords(records: List<Record>) -> Void {
  //# Prepare filter
  planFilters();

  for (record in records) {
    if (record.shouldSkip()) {
      continue;
    } else if (record.isUnknown()) {
      //# Flag unknown record
      markUnknown(record);
    } else {
      //# Persist record
      store(record);
    }
  }

  //# Summarize results
  summarize(records);
}
```

will result in this ControlFlowVisualization:
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("FilterRecords")
        - n1: parent=n0, kind=HeaderContextEnter("Prepare filter")
            - n2: parent=n1, kind=Loop("for (record in records)")
                - n3: parent=n2, kind=BranchGroup("if (record.shouldSkip())")
                    - n4: parent=n3, kind=BranchArm("if (record.shouldSkip())")
                    - n5: parent=n3, kind=BranchArm("else if (record.isUnknown())")
                        - n6: parent=n5, kind=HeaderContextEnter("Flag unknown record")
                    - n7: parent=n3, kind=BranchArm("else")
                        - n8: parent=n7, kind=HeaderContextEnter("Persist record")
        - n9: parent=n0, kind=HeaderContextEnter("Summarize results")

edges_by_src:
    n1: [n9]
    n2: [n3]
    n3: [n4, n5, n7]
```

should get flattened into:
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("FilterRecords")
        - n1: parent=n0, kind=HeaderContextEnter("Prepare filter")
            - n2: parent=n1, kind=Loop("for (record in records)")
	            - n3: parent=n2, kind=BranchGroup("if (record.shouldSkip())")
	            - n4: parent=n2, kind=BranchArm("if (record.shouldSkip())")
	            - n5: parent=n2, kind=BranchArm("else if (record.isUnknown())")
	                - n6: parent=n5, kind=HeaderContextEnter("Flag unknown record")
	            - n7: parent=n2, kind=BranchArm("else")
	                - n8: parent=n7, kind=HeaderContextEnter("Persist record")
        - n9: parent=n0, kind=HeaderContextEnter("Summarize results")

edges_by_src:
    n1: [n9]
    n2: [n3]
    n3: [n4, n5, n7]
```

## Example 15
Shows that a BranchGroup with no headerized work anywhere beneath it disappears entirely after flattening.
```baml pseudocode
function MaybeCompact() -> Void {
  //# Begin maintenance window
  prepareNodes();

  if (shouldCompact()) {
    compact();
  } else {
    relax();
  }

  //# Finish maintenance window
  finishNodes();
}
```

will result in this ControlFlowVisualization:
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("MaybeCompact")
        - n1: parent=n0, kind=HeaderContextEnter("Begin maintenance window")
            - n2: parent=n1, kind=BranchGroup("if (shouldCompact())")
                - n3: parent=n2, kind=BranchArm("if (shouldCompact())")
                - n4: parent=n2, kind=BranchArm("else")
        - n5: parent=n0, kind=HeaderContextEnter("Finish maintenance window")

edges_by_src:
    n1: [n5]
    n3: [n5]
```

which will be flattened like so:
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("MaybeCompact")
        - n1: parent=n0, kind=HeaderContextEnter("Begin maintenance window")
        - n5: parent=n0, kind=HeaderContextEnter("Finish maintenance window")

edges_by_src:
    n1: [n5]
```

## Example 16
Highlights how an `OtherScope` wrapper (such as a transactional block) is removed when it contains no headers, but retained when it provides structure for a nested header
```baml pseudocode
function WriteReport() -> Void {
  //# Start transaction
  beginTransaction();

  let txn_result = {
    //# Persist report body
    persistBody();
  }

  let audit_result = {
    logAudit(); // no header context inside
  }

  //# Publish report
  publish();
}
```

will result in this ControlFlowVisualization:
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("WriteReport")
        - n1: parent=n0, kind=HeaderContextEnter("Start transaction")
            - n2: parent=n1, kind=OtherScope("let txn_result = ")
                - n3: parent=n2, kind=HeaderContextEnter("Persist report body")
            - n4: parent=n1, kind=OtherScope("let audit_result = ")
        - n5: parent=n0, kind=HeaderContextEnter("Publish report")

edges_by_src:
    n1: [n5]
```

should get flattened into:
```
Nodes:
    - n0: parent=None, kind=FunctionRoot("WriteReport")
        - n1: parent=n0, kind=HeaderContextEnter("Start transaction")
			- n3: parent=n1, kind=HeaderContextEnter("Persist report body")
        - n5: parent=n0, kind=HeaderContextEnter("Publish report")

edges_by_src:
    n1: [n5]
    n2: [n3]
    n3: [n5]
```

