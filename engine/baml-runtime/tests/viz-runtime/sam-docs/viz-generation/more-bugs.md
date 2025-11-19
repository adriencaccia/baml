
## DONE- Edges between OtherScope nodes
```baml
function Foo() -> Void {
  //# Do lorem things
  Lorem();
  
  if (true) {
    asdf();
  } else {
    //# inject placeholder 1
    jklmnop();
  }
  
  if (true) {
    asdf();
  } else {
    //# inject placeholder 2
    jklmnop();
  }
  
  //# Prepare ipsum recipes
  Ipsum();
}
```

currently we don't draw an edge between the two `BranchGroup` instances; however, this means that we don't establish a relationship between "inject placeholder 1" and "inject placeholder 2" - therefore we need to draw that edge

## if without else not rendered
```
function Foo() -> Void {
  //# Do lorem things
  Lorem();
  
  if (true) {
    //# asdf
    asdf();
  }
  
  //# Prepare ipsum recipes
  Ipsum();
}
```

currently does not render the `BranchArm/else` node, so the viz will look confusing when it actually happens

## node IDs
we shouldn't use path segments for node IDs, instead just use a monotonic numbering scheme for them and make every node store a `semanticId` which can be used to easily find the corresponding logs and be used for filtering

# notes
- we can't flatten loop nodes because the viz should be able to distinguish
```
while (true) {
  while (true) {
	    //# thing
  }
}
```

## Thursday start
- fix the above bug
- split out a separate react component for the react-flow rendering in the test result preview
- then start working on viz flattening