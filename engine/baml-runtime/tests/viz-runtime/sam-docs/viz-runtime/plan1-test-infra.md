Plan: runtime viz test infra (snapshots)

This is a specification document for a coding agent to set up the testing infra that describes how we'll implement the runtime semantics needed to render a live updating visualization of a function in the BAML language. See /Users/sam/thoughts/sam-projects/viz-runtime/project-goal.md which describes the goal of this project.

Scope
- Set up scaffolding only: directory layout, harness skeleton, serializers, fixtures, and the Vite watcher.
- Use the current event stream as-is; no behavioral changes yet.
- Add three simple baseline tests to lock in the harness: straight-line with headers, simple if/else, simple loop.
- Include a stub VizStateReducer (watch handler client) implementation that we will flesh out later.
- Put tests in under `engine/baml-runtime/tests/viz-runtime/`.

What to build
- Test inputs: single-function files `MyFunction1.baml` where `Main()` calls `MyFunction1()` so you can pass args via `Main`.
- Snapshot artifact using docs.rs/insta per test case:
	- use /Users/sam/baml7/engine/baml-runtime/src/control_flow/tests.rs test_snapshot as the model for how snapshot testing should be set up for viz-runtime semantics
	- Raw event stream emitted by the runtime to `watch_handler`.
	- For every event, the client’s context stack after applying it.
	- For every event, the emitted `(lexical_id, new_state)` deltas, where `new_state ∈ { not_running, running, completed }`.

Implementation sketch
- Harness location: `engine/baml-runtime/tests/viz-runtime/`.
- Data flow:
  1) Compile/load `MyFunction1.baml`.
  2) Execute via runtime (and async_vm_runtime when toggled) with a `watch_handler` that records events.
  3) Pass recorded events through the stubbed VizStateReducer to produce placeholder `(lexical_id, new_state)` updates; fill in real behavior in later passes.
  4) Serialize three streams (events, stack snapshots, updates) to deterministic snapshot files (e.g., JSONL) alongside the `.baml` input.
- Determinism: strip timestamps/IDs, normalize ordering, and use stable fixture paths so diffs are reviewable.
- Commands: expose a `cargo test -p baml-runtime viz_runtime::tests -- --nocapture` mode that regenerates/validates snapshots (can gate updates behind an env var similar to insta).

Web preview
- Build a tiny Vite/React app (or similar) that:
  - Watches the snapshot directory for changes (Vite dev server gives HMR out of the box).
  - Loads every snapshot file in the directory and displays event timeline, stack state, and node state changes.
  - Lives colocated with the Rust snapshot output location (probably need to add it to `pnpm-workspace.yaml` in /Users/sam/baml7)
