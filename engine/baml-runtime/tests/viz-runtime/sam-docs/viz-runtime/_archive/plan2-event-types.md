Plan: runtime viz event type definitions

This is a specification for a coding agent to introduce typed control-flow context events that baml-runtime will emit via `watch_handler`, aligned with `/Users/sam/thoughts/sam-projects/viz-runtime/context-event-schema.md` and the live viz architecture doc.

Scope
- Define Rust types for control-flow context events and add the new watch stream variant(s) in a **new crate `baml-viz-events`** that both `baml-compiler` and `baml-runtime` can depend on. No runtime emission logic changes yet—just type plumbing and serialization.
- Keep changes focused on Rust code; WASM/JS bridge is intentionally left untouched.

Do not do in this pass
- No compiler/VM/interpreter instrumentation, no VizStateReducer/UI wiring, no snapshot/harness redesign beyond compile fixes, no backward-compat gates, no WASM/JS changes.

Schema essentials (inline Rust sketch; replaces the deleted context-event-schema.md)
```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum VizExecDelta {
    Enter,
    Exit,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum VizNodeType {
    FunctionRoot,
    HeaderContextEnter,
    BranchGroup,
    BranchArm,
    Loop,
    OtherScope,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VizExecEvent {
    pub event_type: VizExecDelta,
    pub node_type: VizNodeType,
    
    pub node_type: RuntimeNodeType,
    pub label: String,
    pub header_level: Option<u8>,
}
```
- VizStateReducer semantics: VizStateReducer will consume these events as they're emitted by the baml runtime. HeaderContextEnter events will be **enter-only**; client should pop headers until the top header level is below the incoming header’s level, then push the new header (no header exits). Non-header nodes (FunctionRoot, BranchGroup, BranchArm, Loop, OtherScope) emit explicit Enter/Exit pairs and follow LIFO push/pop per `call_id`. Stack depth is measured after applying the event. Exits must match the last lexical_id on the stack.
- JSON example remains the same shape as before, with `type` = `"control_flow_context"` and `event` = `"enter" | "exit"` (headers only use `enter`).

Plan
1) Add `baml-viz-events` crate
   - New crate with only `serde` (and std) dependencies to avoid cycles.
   - Expose `ControlFlowContextEvent` struct (fields from `context-event-schema.md`), `ControlFlowEvent` enum (`Enter | Exit` only), and `RuntimeNodeType` enum mirroring viz nodes (FunctionRoot, HeaderContextEnter, BranchGroup, BranchArm, Loop, OtherScope). Keep field names JSON-friendly and derive `Clone`, `Debug`, `Serialize`, `Deserialize`, `PartialEq`.

1) Integrate the new types into watch notifications and all handlers.
   - Add `WatchBamlValue::VizExecState(VizExecEvent)` in `engine/baml-compiler/src/watch/watch_event.rs` and re-export the event types from `baml-viz-events`.
   - Update `WatchNotification` constructors/helpers and `Display` to handle the new variant (e.g., `(context enter) <lexical_id>`).
   - Touch all match sites so compilation succeeds: `engine/baml-runtime/src/async_vm_runtime.rs`, `async_interpreter_runtime.rs`, `lib.rs`, CLI REPL, and `engine/baml-runtime/tests/viz-runtime.rs` (map to a stub EventRecord entry for now).
   - Do not make any logic changes in how the handlers consume the new events.

5) Basic serialization tests
   - Add a minimal serde round-trip test for `ControlFlowContextEvent` inside the new crate.
   - Ensure the runtime test harness accepts the new variant without panicking (even if handled as a placeholder).
