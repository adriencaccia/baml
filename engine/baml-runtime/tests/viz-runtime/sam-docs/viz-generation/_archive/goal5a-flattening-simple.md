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
            - n2: parent=n1, kind=BranchGroup("if (true)")
            - n3: parent=n1, kind=BranchArm("if (true)")
            - n4: parent=n1, kind=BranchArm("else")
        - n5: parent=n0, kind=HeaderContextEnter("Prepare ipsum recipes")

edges_by_src:
    n1: [n5]
    n2: [n3, n4]
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
    //# Handle warning
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
                - n3: parent=n2, kind=BranchArm("if (level == \"urgent\")")
                    - n4: parent=n3, kind=HeaderContextEnter("Handle urgent")
                    - n5: parent=n3, kind=HeaderContextEnter("Page escalation ladder")
                - n6: parent=n2, kind=BranchArm("else if (level == \"warning\")")
                    - n7: parent=n6, kind=HeaderContextEnter("Handle warning")
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
            - n3: parent=n1, kind=BranchArm("if (level == \"urgent\")")
                - n4: parent=n3, kind=HeaderContextEnter("Handle urgent")
                - n5: parent=n3, kind=HeaderContextEnter("Page escalation ladder")
            - n6: parent=n1, kind=BranchArm("else if (level == \"warning\")")
                - n7: parent=n6, kind=HeaderContextEnter("Handle warning")
            - n8: parent=n1, kind=BranchArm("else")
                - n9: parent=n8, kind=HeaderContextEnter("Handle normal")
            - n10: parent=n1, kind=BranchGroup("if (level.includes(\"phone\"))")
            - n11: parent=n1, kind=BranchArm("if (level.includes(\"phone\"))")
            - n12: parent=n1, kind=BranchArm("else")
        - n13: parent=n0, kind=HeaderContextEnter("Finalize evaluation")

edges_by_src:
    n1: [n13]
    n2: [n3, n6, n8]
    n4: [n5]
    n10: [n11, n12]
```
