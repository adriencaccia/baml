# Live Visualization Architecture & Workstreams

## Goal
Enable the IDE/runtime to highlight the currently executing node inside the existing `.function_graph_v2()` visualization. While BAML code runs we must emit structured context events that let the UI map runtime execution back onto the static graph via `lexical_id`.

## Inconsistencies Resolved
1. `project-goal.md:5-7` only asks for `ContextExit` on scope nodes even though the static traversal (`HirTraversalContext`) relies on both enter/exit events for *headers* and *scopes*. **Decision:** emit `ContextEnter`/`ContextExit` for every visualization node (function root, headers, branch groups/arms, loops, synthetic scopes). Header exits happen when their lexical scope ends (next header of same or lower level, or block exit).
2. `project-goal.md:9` assumes `lexical_id` is known during execution, but the runtime currently receives only `block_name` and `level`. **Decision:** at compile time, compute the exact lexical id and node metadata for every structural construct and embed it into the bytecode’s `BlockNotification` records so the VM/interpreter can just copy it into events.
3. `project-goal.md:5` references `watch_handler`, yet today `WatchNotification::Header` carries no stack semantics or exit events, and the WASM bridge serializes `value` as a JSON string. **Decision:** introduce a first-class `ControlFlowContext` variant on `WatchBamlValue` that transports typed payloads all the way to JS; keep the legacy string field for backward compatibility until the UI migrates.
4. Only headers currently trigger `NotifyBlock` instructions (`codegen.rs:560`), so loops/ifs never surface in watch events. **Decision:** extend codegen to insert paired enter/exit notifications for every lexical frame (`Function`, `Header`, `BranchGroup`, `BranchArm`, `Loop`, `OtherScope`). `BlockNotificationType` already enumerates most variants; we will expand it if needed for branch groups/arms.

## Runtime Data Flow
1. **Compiler phase:** when compiling each function, reuse `ControlFlowVisualization` (or a shared helper) to produce a deterministic `RuntimeContextTable`. Each entry contains `lexical_id`, `node_type`, `label`, `span`, header level (when applicable), and bytecode offsets where the node begins/ends.
2. **Bytecode instrumentation:** for every context entry/exit we emit a `NotifyBlock` instruction that references the matching table index. We duplicate the entry for exit events (or mark `is_enter` accordingly). The same metadata table must be available to the interpreter backend.
3. **VM/interpreter runtime:** when a `NotifyBlock` executes, emit a `VmWatchNotification::Context` that carries the metadata blob + `event_kind=Enter|Exit` and a monotonically increasing counter per function invocation.
4. **`baml-runtime` layer:** convert the VM/interpreter notification into `watch::WatchNotification::ControlFlowContext`. This struct stores the metadata (`lexical_id`, `node_type`, `label`, `span`, `header_level`, `timestamp`, `call_id`, `stack_depth`, `event_id`), plus bookkeeping (e.g., parent lexical id) so clients can rebuild the stack without guessing.
5. **WASM/CLI bridge:** extend `engine/baml-schema-wasm/src/runtime_wasm/mod.rs` to map `ControlFlowContext` into a JS object with top-level fields instead of a JSON string. Provide versioned capability flags so older frontends ignore unknown events gracefully.
6. **Client/UI:** watch handler consumers treat the context events as a stream of deltas. They resolve `lexical_id` against `.function_graph_v2()` nodes and mark the latest `ContextEnter` without a matching exit as “active”.

## Workstreams for Coding Agents
### 1. Control-flow metadata extraction (`engine/baml-runtime/src/control_flow.rs`, compiler pipeline)
- Factor the lexical-id builder (`build_lexical_id`, `PathSegment`, etc.) into a shared crate so codegen can ask for the lexical id/parent relationships without duplicating logic.
- Produce a `RuntimeContextTable` alongside the existing graph: `IndexMap<u32, RuntimeContextNode>` keyed by deterministic ids.
- Decide how to encode branch groups/arms and anonymous scopes so runtime metadata mirrors the visualization.

### 2. Bytecode & interpreter instrumentation (`engine/baml-compiler/src/codegen.rs`, `engine/baml-vm/src/bytecode.rs`)
- Extend `BlockNotification` with `lexical_id: String`, `node_type: RuntimeNodeType`, `label: String`, `span: SerializedSpan`, `header_level: Option<u8>`, `parent_lexical_id: Option<String>`.
- Generate paired notifications for enter/exit events. For scopes that do not have explicit AST nodes (e.g., implicit anonymous scopes), allocate them in the runtime context table so we can still emit matching exits.
- Ensure both the async interpreter and VM runtimes consume the richer metadata (the interpreter may not execute bytecode but should emit equivalent notifications).

### 3. Runtime watch emission (`engine/baml-runtime/src/async_vm_runtime.rs`, `src/async_interpreter_runtime.rs`)
- Add a new VM notification variant instead of reusing `WatchNotification::Block` so we can pass the extra fields without lossy conversions.
- Maintain a per-invocation stack to attach `stack_depth` and detect mismatched exits. Log (or panic in debug) when the VM emits an exit for the wrong lexical id—it signals a bug in instrumentation.

### 4. Watch handler contract (`engine/baml-compiler/src/watch/watch_event.rs`, `engine/baml-schema-wasm/...`)
- Introduce `WatchBamlValue::ControlFlowContext(ContextEvent)` and plumb it through every place that matches on `WatchBamlValue`.
- Update the WASM bridge to emit objects like:
  ```json
  {
    "type": "control_flow_context",
    "event": "enter",
    "function_name": "checkout",
    "lexical_id": "checkout|hdr:verify-payment:1",
    "node_type": "HeaderContextEnter",
    "label": "//# Verify payment",
    "span": { "file": "baml_src/checkout.baml", "start": 42, "end": 84 },
    "stack_depth": 3,
    "timestamp": 1717000000
  }
  ```
- Preserve the old `value` string for non-context notifications so existing UIs keep working.

### 5. UI/client integration (JS playground & IDE)
- Build a lightweight stack reconciler that consumes the event stream and highlights nodes. It should:
  - Reset the stack whenever a new function invocation begins.
  - Handle out-of-order events defensively (log & drop) but assume the VM sends them in order.
  - Debounce updates to avoid thrashing the React Flow graph.

## Testing Strategy
- Extend `engine/baml-runtime/src/control_flow/tests.rs` to snapshot the new runtime context table to ensure lexical ids stay in sync with `.function_graph_v2()`.
- Add VM integration tests (`engine/baml-vm/tests/watch.rs`) that execute small programs and assert on the emitted enter/exit sequence.
- Add WASM tests that simulate watch notifications and verify the JS bridge produces the documented payload.
- Backfill UI tests (Playwright or Vitest) to ensure live highlighting follows the runtime execution order.

## Follow-ups / Open Questions
- How to expose capability flags so clients can opt-in to the richer protocol? Proposal: emit a `watch_protocol_version` field once per session via an initial notification.
- How to handle async/await edges (e.g., contexts suspended mid-stack)? Need a policy for whether `ContextExit` fires immediately before suspension or only after resume completes.
