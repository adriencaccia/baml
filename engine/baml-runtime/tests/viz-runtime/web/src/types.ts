export type LexicalState = "not_running" | "running" | "completed";

export interface EventRecord {
  kind: string;
  function: string;
  variable?: string | null;
  channel?: string | null;
  stream_id?: string | null;
  lexical_id?: string | null;
  header?: {
    level: number;
    title: string;
  };
  viz_event?: VizExecEvent | null;
  value?: unknown;
  is_stream: boolean;
}

export interface StateUpdate {
  node_id: number;
  lexical_id: string;
  new_state: LexicalState;
}

export type VizExecDelta = "enter" | "exit";

export interface VizExecEvent {
  event: VizExecDelta;
  node_id: number;
  node_type: string;
  lexical_id: string;
  path_segment: {
    kind: string;
    slug?: string;
    ordinal: number;
  };
  label: string;
  header_level?: number | null;
}

export interface SnapshotRow {
  watch_event: EventRecord;
  stack_after?: string[] | null;
  emitted_events?: StateUpdate[] | null;
}

export interface SnapshotEntry {
  fixture: string;
  inputFile?: string;
  rows: SnapshotRow[];
}

export interface CombinedRow {
  index: number;
  watchEvent: EventRecord;
  stackAfter: string[];
  emittedEvents: StateUpdate[];
}
