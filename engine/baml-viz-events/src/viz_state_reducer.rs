use serde::{Deserialize, Serialize};

use crate::{PathSegment, RuntimeNodeType, VizExecEvent};

/// Tracking state for a lexical node while replaying runtime events.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LexicalState {
    NotRunning,
    Running,
    Completed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateUpdate {
    /// Stable node id (matches control_flow.rs pre-order allocation).
    pub node_id: u32,
    pub lexical_id: String,
    pub new_state: LexicalState,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Frame {
    pub node_id: u32,
    pub lexical_segment: PathSegment,
    pub lexical_id: String,
    pub node_type: RuntimeNodeType,
    pub label: String,
    pub header_level: Option<u8>,
}

/// Simple reducer that models a lexical node's lifecycle.
#[derive(Default, Debug)]
pub struct VizStateReducer {
    frames: Vec<Frame>,
}

impl VizStateReducer {
    /// Apply a single viz exec event and return the resulting state updates.
    ///
    /// `function_name` is currently unused; lexical ids are supplied on the viz event but kept
    /// for compatibility with callers that already pass it through.
    pub fn apply(&mut self, _function_name: &str, viz_event: &VizExecEvent) -> Vec<StateUpdate> {
        match viz_event.event {
            crate::VizExecDelta::Enter => self.handle_enter(viz_event),
            crate::VizExecDelta::Exit => self.handle_exit(viz_event),
        }
    }

    /// Current stack of frames (root at index 0).
    pub fn dump(&self) -> Vec<Frame> {
        self.frames.clone()
    }

    /// Current lexical_id stack (root at index 0).
    pub fn lexical_stack(&self) -> Vec<String> {
        self.frames.iter().map(|f| f.lexical_id.clone()).collect()
    }

    fn handle_enter(&mut self, viz_event: &VizExecEvent) -> Vec<StateUpdate> {
        let mut updates = Vec::new();

        // Pop headers at or deeper than the incoming header level; this mirrors
        // HirTraversalContext::pop_headers_to_level.
        if viz_event.node_type == RuntimeNodeType::HeaderContextEnter {
            let new_level = viz_event.header_level.unwrap_or(1).max(1);
            while let Some(top) = self.frames.last() {
                if top.node_type == RuntimeNodeType::HeaderContextEnter {
                    let top_level = top.header_level.unwrap_or(1);
                    if top_level >= new_level {
                        let frame = self.frames.pop().expect("frame to exist");
                        updates.push(StateUpdate {
                            node_id: frame.node_id,
                            lexical_id: frame.lexical_id,
                            new_state: LexicalState::Completed,
                        });
                        continue;
                    }
                }
                break;
            }
        }

        let lexical_id = viz_event.lexical_id.clone();

        let frame = Frame {
            node_id: viz_event.node_id,
            lexical_segment: viz_event.path_segment.clone(),
            lexical_id: lexical_id.clone(),
            node_type: viz_event.node_type.clone(),
            label: viz_event.label.clone(),
            header_level: viz_event.header_level,
        };
        self.frames.push(frame);

        updates.push(StateUpdate {
            node_id: viz_event.node_id,
            lexical_id,
            new_state: LexicalState::Running,
        });

        updates
    }

    fn handle_exit(&mut self, viz_event: &VizExecEvent) -> Vec<StateUpdate> {
        let mut updates = Vec::new();
        let mut popped: Vec<Frame> = Vec::new();
        let mut found = false;

        while let Some(frame) = self.frames.pop() {
            let matches = frame.lexical_segment == viz_event.path_segment;
            popped.push(frame);
            if matches {
                found = true;
                break;
            }
        }

        if !found {
            // No matching frame; restore stack and emit nothing.
            for frame in popped.into_iter().rev() {
                self.frames.push(frame);
            }
            return updates;
        }

        for frame in popped {
            updates.push(StateUpdate {
                node_id: frame.node_id,
                lexical_id: frame.lexical_id,
                new_state: LexicalState::Completed,
            });
        }

        updates
    }
}

/// Encode a full lexical id from function name + segments (matches control_flow.rs).
pub fn encode_segments(function: &str, segments: &[PathSegment]) -> String {
    let mut encoded = String::from(function);
    for segment in segments {
        encoded.push('|');
        encoded.push_str(&segment.encode());
    }
    encoded
}
