## Plan: implement viz exec event emission in compiler + VM

This is one of many specification documents that describe the changes to be made to the BAML runtime and compiler necessary to render a live updating visualization of a BAML function, specifically one which updates as execution progresses. See /Users/sam/thoughts/sam-projects/viz-runtime/project-goal.md which describes the goal of this project.

The architecture we're intending to land on will look as follows:
- the BAML runtime will, for every `//#` `//##` `//###` -type header, emit "header context entered" events via the watch events callback
- the BAML runtime will, for every scope (if, else-if, else, for, while), emit "scope entered" and "scope exited" events via the watch events callback
- the watch events callback will then consume the stream of VizExecEvents emitted by the runtime, maintain a context frame stack in VizStateReducer, then for each event received from the baml runtime, emit one or more events to typescript describing the new visualization state of each node (NotRunning/Running/Completed)

Our job right now is to implement event emitting in the BAML runtime for header context entered, scope entered, and scope exited events.

### Phases (run `cargo build` after each)
- Phase 1: Bytecode and metadata plumbing
  - Add `VizEnter/VizExit` instructions and `viz_nodes` field on `Function` with serialization/debug updates.
  - Run `cargo build` to ensure core types wire up.
- Phase 2: VM consumption
  - Implement VM handling for `VizEnter/VizExit` emitting `VizExecEvent` via watch.
  - Run `cargo build` to validate VM changes compile.
- Phase 3: Compiler node construction
  - Build `viz_nodes` during THIR→bytecode mirroring `control_flow.rs`; hook into Function struct.
  - Run `cargo build` to verify compiler changes.
- Phase 4: Instruction emission
  - Insert `VizEnter/VizExit` at all enter/exit points per rules; optional debug assertions.
  - Run `cargo build` to confirm full pipeline compiles.
- Phase 5: Tests
  - Add control-flow/viz emission tests in VM/compiler harness.
  - Run `cargo build`/`cargo test` for coverage.

### Goal
- Make the VM emit control-flow context events (enter/exit, plus header enter-only) in sync with the viz graph. Use the types defined in `engine/baml-viz-events`.

### Scope
- Compiler/bytecode generation and VM runtime emission only. No UI/TS changes, no WASM bridge changes.
- Cover FunctionRoot, HeaderContextEnter, BranchGroup, BranchArm, Loop, OtherScope (synthetic scopes), including else-if chains and trailing block expressions per control_flow.rs semantics.

### Deliverables
- New bytecode instructions for viz enter/exit.
- Per-function viz metadata table (ids, parents, labels, header levels) stored in the compiled `Function`.
- Compiler changes to emit correct enter/exit instructions at all control-flow edges.
- VM changes to decode the instructions and emit `VizExecEvent` via the watch callback.
- Tests that assert emitted sequences for key control-flow shapes.

### Design summary (trust bytecode, no VM viz stack)
- Precompute viz nodes per function during THIR→bytecode, mirroring `control_flow.rs`:
  - Build `viz_nodes: Vec<VizNodeMeta { id, parent, node_type, label, header_level }>` with lexical ids/parents matching the viz builder.
  - Nodes: FunctionRoot (1 per function), BranchGroup (1 per if/else-if/else chain), BranchArm (1 per arm), Loop (1 per loop construct), OtherScope (only when control_flow.rs would wrap a block expr), HeaderContextEnter (one per header stmt).
  - Labels match control_flow.rs formatting: `if (...)`, `else if (...)`, `else`, `while (<cond>)`, `for (<id> in <iter>)`, `for (<cond>)`/`for (...)`, `let name = { ... }` etc. Header nodes carry `header_level`.
- Add bytecode instructions `VizEnter(idx)` / `VizExit(idx)` where `idx` indexes `function.viz_nodes`.
- VM simply reads `function.viz_nodes[idx]` and emits the `VizExecEvent { delta, node_type, label, header_level, parent/id }` via the existing `WatchNotification` enum (add enter/exit as a viz variant, not a new notification type). No VM-maintained viz stack if bytecode is correct.

### Compiler tasks
- Data plumbing:
  - Extend the compiled `Function` struct to carry `viz_nodes: Vec<VizNodeMeta>`; ensure serialization/deserialization if needed.
  - Add the new instructions to `bytecode.rs` and any display/debug helpers.
- Node construction:
  - During function codegen, build the viz tree once (lexical), mirroring `control_flow.rs::emit_*`. Else-if chains share one BranchGroup id; each arm gets its own BranchArm id. OtherScope nodes only for block exprs that control_flow.rs wraps (including trailing expr blocks).
- Instruction emission (use the precomputed node indices):
  - FunctionRoot: emit `VizEnter(root)` before first body stmt; emit `VizExit(root)` on every return path (explicit and implicit root return).
  - Headers: emit `VizEnter(header_node)` at the header statement; no exit.
  - Branches: emit BranchGroup enter before first condition; for the taken arm emit BranchArm enter at arm start and BranchArm exit after arm body; emit BranchGroup exit immediately after the taken arm’s exit.
  - Loops: emit Loop enter at body start each iteration; emit Loop exit on every path leaving the iteration body: normal fallthrough before back-edge jump, `continue`, `break`, and `return` inside the loop.
  - OtherScope/synthetic blocks: emit enter at block start and exit at block end (including trailing expr blocks that are wrapped).
  - Scope exit injection points (all exit instructions go right before the control transfer):
    - Explicit return: after expression codegen, before `Instruction::Return`, emit exits for all currently-open viz scopes (OtherScope, BranchArm, Loop iteration, BranchGroup, FunctionRoot—root last).
    - Implicit return (root scope end): same full unwind before the implicit `Return` in `exit_scope` when depth == 0.
    - `continue`: after `emit_scope_drops`, emit exits for scopes being abandoned in the current iteration (inner OtherScopes/BranchArm + Loop iteration), then emit the `Jump` back to loop head.
    - `break`: after `emit_scope_drops`, emit exits for scopes being abandoned through the loop iteration (OtherScopes/BranchArm + Loop iteration), then emit the `Jump` that patches to after the loop.
    - Loop fallthrough: emit Loop exit just before the back-edge `Jump`.
  - Optional: during codegen maintain a temporary viz-scope stack for debug assertions (LIFO exits, empty at function end).

### VM tasks
- Add handling for `VizEnter/VizExit` in `vm.rs`: fetch `viz_nodes[idx]` from the current function, construct `VizExecEvent` (delta Enter/Exit, node_type from metadata, label, header_level, id/parent) and send through the watch notification path (same as plan2 variant). No runtime stack manipulation required.
- Ensure `Function` deserialization/lookup makes `viz_nodes` available in the VM.
- Optionally log/assert if `idx` is out of bounds to catch compiler bugs.

### Testing
- Add VM/control-flow tests that execute bytecode and capture emitted VizExecEvents; assert sequences for:
  - Straight-line function with headers.
  - If/else-if/else (only taken arm emits; group/arm enter/exit order).
  - While/for with continue and break (per-iteration enter/exit on all paths).
  - Early return inside loop.
  - Synthetic OtherScope (let with block RHS, trailing block expr) enter/exit.
- Consider a compiler-side unit test that builds `viz_nodes` for a representative THIR snippet and matches control_flow.rs’ expected structure/labels.
