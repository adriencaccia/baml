## Plan: implement VizStateReducer (event stream → node state updates)

This spec tells a coding agent how to finish the VizStateReducer so that the streamed `VizExecEvent`s emitted by the runtime are transformed into a deterministic list of `StateUpdate { lexical_id, new_state ∈ { not_running, running, completed } }` notifications. `VizExecEvent`s now carry only the `PathSegment` of the node being entered/exited (plus node_type/label/header_level)—they do **not** include a precomputed `lexical_id`. The reducer must reconstruct lexical IDs from the event order and segments. See `engine/baml-viz-events/src/viz_state_reducer.rs` (stub) and the test harness in `engine/baml-runtime/tests/viz-runtime.rs`. This reducer becomes the single source of truth for the client view state (Rust tests + `tests/viz-runtime/web/`), and should stay a pure state-machine over a stack of frames—no separate `node_states` map is needed.

### Scope and non-goals
- Only implement the reducer + tests/harness updates; do not change runtime emission semantics here (plan3rev2 covers that).
- Assume the runtime now emits lexicalized `VizExecEvent`s for all scope enters/exits (`function_root`, `branch_group`, `branch_arm`, `loop`, `other_scope`) and only enter events for headers (`header_context_enter`).
- Keep everything in Rust for now; the TS types already mirror the structures.

### Event stream assumptions (input contract)
- Each `VizExecEvent` has `{ event: enter|exit, node_type, path_segment, label, header_level? }` (no `lexical_id` field).
- The `path_segment` contains the encoded segment type + slug + ordinal emitted by the compiler, mirroring `control_flow.rs` `PathSegment`.
- `lexical_id` must be synthesized by the reducer using the function name (from the watch event) and the current stack of segments.
- Header nodes only emit enters; all other node types emit paired enter/exit events.
- Event order reflects execution order; exits always happen after their matching enter (but be defensive to avoid panics).

### Reducer responsibilities (stack-only model)
- Maintain `frames: Vec<Frame>` as the active context stack in execution order (root at bottom), where `Frame` holds `{ lexical_segment, lexical_id, node_type, label, header_level }`.
- `apply(&VizExecEvent) -> Vec<StateUpdate>` should:
  - Update `frames` according to the event (see rules below).
  - Emit `StateUpdate`s reflecting transitions implied by stack changes:
    - Enter: emit `running` for the entered node.
    - Exit/pop: emit `completed` for every frame popped.
  - Emit updates in the order transitions happen while handling the event (e.g., if unwinding pops multiple frames, emit updates for each popped frame in pop order, then the newly-entered frame).
- `dump()` should return the current frames
- Provide a helper to synthesize the current top-of-stack lexical_id (or full stack) so the watch handler can attach lexical_ids to **all** watch events, including non-viz events (headers/value/stream). Use the current stack’s segments + function name to encode IDs identically to `control_flow.rs::encode_segments`.

### Stack manipulation rules (match `HirTraversalContext` heuristics)
- Push on every enter (after any pre-unwind).
- Pop on every exit until the matching path_segment is found (inclusive); mark every popped frame `completed`.
- Header enter heuristic (because runtime never emits header exits):
  - Treat `header_level` as `max(1, header_level.unwrap_or(1))`.
  - Before pushing a new header frame, pop **only** consecutive header frames from the top whose `header_level >= new_level`, marking each `completed`. This mirrors `HirTraversalContext::pop_headers_to_level(level - 1)` in `engine/baml-runtime/src/control_flow.rs`.
  - Do not pop non-header frames during this heuristic; headers nested under scopes are still unwound when the surrounding scope exits.
- Exit heuristic:
  - When processing an exit event for any node type, pop frames until a frame with the same `path_segment` is found; convert each popped frame to `completed`.
  - If no matching frame is found, best-effort pop nothing and return an empty update list (or log via `debug_assert!`); avoid panicking in release.
- Re-entry handling:
  - A node previously marked `completed` can transition back to `running` on a fresh enter (e.g., loop iterations).
  - Ignore no-op transitions (e.g., entering a node that is already `running` without an intervening exit).

### State transition rules
- `enter` ⇒ emit `running` for the node being pushed.
- `exit` or header-unwind pop ⇒ emit `completed` for each node being popped.
- No explicit `not_running` updates are needed; absence of a node implies not running until first enter.

### Test/harness updates (Rust + snapshots)
- Extend `VizStateReducer` dump to return the current frames (and optionally the lexical_id stack). Update the watch handler in `engine/baml-runtime/tests/viz-runtime.rs` to:
  - Capture the state snapshot after each event (stack/frames) and the synthesized lexical_id for that event (even when the event isn’t a viz enter/exit).
  - Store them in the insta snapshot rows (`stack_after` should reflect the lexical_id stack; `state` should include frames mirroring `tests/viz-runtime/web/src/types.ts`).
- Add/refresh fixtures to assert reducer behavior:
  1) Straight-line headers: entering `//# Start` then `//# Finish` at the same level should complete `Start` when `Finish` enters.
  2) Nested headers (`//#` then `//##`) then a sibling `//#` should pop only the nested headers per the level heuristic.
  3) Header inside a scope (e.g., inside an if arm): when the arm exits, the header should be completed even without a header-exit event.
  4) Loop with per-iteration enter/exit: loop node transitions `running → completed → running` across iterations; inner headers should complete each iteration.
- Keep existing simple fixtures (`straight_line_with_headers.baml`, `simple_if_else.baml`, `simple_loop.baml`) but update snapshots to reflect real lexical_ids and reducer transitions once plan3 changes land.

### Deliverables
- Implemented reducer logic in `engine/baml-viz-events/src/viz_state_reducer.rs` with tests if practical.
- Updated viz-runtime test harness + insta snapshots showing stack/frames evolution and emitted state updates.
- Brief inline comments/doc to explain the header-level unwind heuristic referencing `control_flow.rs`.
