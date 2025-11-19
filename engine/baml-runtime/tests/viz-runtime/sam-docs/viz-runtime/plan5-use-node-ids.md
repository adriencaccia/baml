## Plan 5: add stable node IDs to viz pipeline (compiler → runtime → reducer)

Goal: Carry a stable `node_id` (u32) for every viz node end-to-end, alongside `lexical_id`. `node_id` disambiguates nodes when `lexical_id` is insufficient (e.g., duplicated labels/segments), and must be attached to all relevant watch events and `StateUpdate`s emitted by `VizStateReducer`.

Context: Current pipeline uses `lexical_id` reconstructed from path segments; `VizExecEvent` carries `PathSegment`, `node_type`, `label`, `header_level`. `StateUpdate` only carries `lexical_id`. We need to introduce `node_id` sourced from compiler ordering that matches `control_flow.rs` node ordering.

### Requirements
- Assign deterministic `node_id` per viz node during THIR→bytecode (same order as `control_flow.rs::ControlFlowVizBuilder`).
- Persist `node_id` in serialized `viz_nodes` metadata on compiled `Function`.
- Emit `node_id` in VM’s `VizExecEvent`.
- `VizStateReducer` must thread `node_id` through frames and state updates: `Frame` includes `node_id`, `StateUpdate` includes `node_id`.
- Watch notifications delivered to clients include both `node_id` and `lexical_id`. Snapshots should record both so post-hoc log filtering can use `node_id` while UI can still show `lexical_id`.

### Data model changes (Rust crate: baml-viz-events)
- `StateUpdate` ⇒ add `node_id: u32`.
- `Frame` ⇒ add `node_id: u32`.
- `VizExecEvent` ⇒ add `node_id: u32` (in addition to existing `path_segment` etc).
- Ensure serde round-trips for new field; update TS type definitions under `tests/viz-runtime/web/src/types.ts`.

### Compiler/bytecode generation
- In THIR→viz builder (likely `engine/baml-compiler/src/viz_builder.rs` or analogous bytecode stage):
  - Assign incrementing `node_id` (`u32`) in the same visitation order as `control_flow.rs`. Reuse the same notion of `ControlFlowVizBuilder::allocate_id` order to keep parity.
  - Store `node_id` on each `VizNodeMeta` entry in `function.viz_nodes` (Rust struct in `baml-vm/src/types.rs` or wherever `VizNodeMeta` is defined).
  - Update serialization/deserialization/display for `VizNodeMeta` to include `node_id`.
  - When emitting `VizEnter` / `VizExit`, continue to use index into `viz_nodes`; VM will read `node_id` from the metadata.

### VM/runtime emission
- In `engine/baml-vm/src/vm.rs::build_viz_exec_event`, include `node_id` from the looked-up `viz_nodes[index]` in the `VizExecEvent`.
- Watch display (`WatchNotification` fmt) should show `node_id` for viz events (e.g., `n{node_id}` or `id:{node_id}`).

### Reducer (baml-viz-events)
- Frames store `node_id`; when pushing on enter, pull from `viz_event.node_id`.
- `StateUpdate` emitted on enter/exit/header-unwind includes both `node_id` and `lexical_id`.
- Stack tracking remains path-segment based for unwinding; `node_id` purely threads through for uniqueness.

### Test harness & snapshots (engine/baml-runtime/tests/viz-runtime)
- `EventRecord` / `StreamSnapshot` should capture `node_id` present in `VizExecEvent`.
- `StateUpdate` in snapshots now has `node_id`.
- Web app types/rendering updated to display `node_id` alongside events/updates (if needed; at minimum include in JSON).
- Regenerate insta snapshots after wiring to lock in new fields.

### TypeScript (tests/viz-runtime/web)
- Update `VizExecEvent` and `StateUpdate` types: add `node_id: number`.
- Update renderer to show `node_id` where helpful; at minimum ensure the JSON includes it.

### Control-flow parity & validation
- Add a unit/integration check to assert `node_id` ordering matches `control_flow.rs` node ordering for a known fixture (optional but recommended). `control_flow.rs` assigns `node_id` in pre-order (prefix): each node is allocated just before descending into its children; IDs are a single increasing counter starting at 0 per function.
- VM should handle out-of-bounds/invalid `node_id` gracefully (debug assertions/logs optional).

### Out of scope
- No UI/visual changes beyond showing new fields in debug views.
- No change to `lexical_id` synthesis semantics; it stays as-is for display.

### Deliverables
- Compiler/VM/reducer updated to carry `node_id` end-to-end.
- Snapshot harness and web types updated; snapshots regenerated to include `node_id` in viz events and state updates.
- Brief inline comments/notes describing `node_id` purpose and ordering relationship to `control_flow.rs`.
