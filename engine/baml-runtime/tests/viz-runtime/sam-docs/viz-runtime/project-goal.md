
The BAML compiler/runtime currently exposes `.function_graph_v2()` which allows generating a static visualization of a given baml expression function: this logic is described in /Users/sam/thoughts/sam-projects/viz-generation/background2.md.

Our job now is to make it possible to, while a BAML program is executing, show the current execution state in the static visualization. We'll do this by:
- changing the structure of the event stream sent to `watch_handler` (see `engine/baml-schema-wasm/src/runtime_wasm/mod.rs`), specifically
- just like how `HirTraversalContext` in `engine/baml-runtime/src/control_flow.rs` allows us to build the static visualization by tracking when we "enter" or "exit" either a `//#` `//##` `//###` header or a scope (if, else-if, else, for, while, and anonymous scopes),
- at runtime, whenever encountering a `//#` `//##` `//###` header, emit a `ContextEnter` event , and whenever encountering a new scope enter or exit (an if, else-if, else, for, while, or anonymous scope) we'll also emit `ContextEnter` and `ContextExit` events

The idea is that the client should be able to consume the `ContextEnter` and `ContextExit` events and use their contents to uniquely identify the currently executing visualization node, by resolving the `lexical_id` .