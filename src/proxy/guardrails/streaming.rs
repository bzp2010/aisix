use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct StreamCheckpoint(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum StreamGuardrailDecision {
    Pending,
    Allow { approved_through: StreamCheckpoint },
    Block { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WholeResponseReplayAction<Chunk> {
    Buffered(Chunk),
    Emit(Chunk),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WholeResponseReplayFinalize {
    NeedsGuardrailCheck,
    Finished,
}

#[derive(Debug)]
pub(crate) struct WholeResponseReplay<Chunk> {
    next_checkpoint: u64,
    buffered_chunks: VecDeque<(StreamCheckpoint, Chunk)>,
}

impl<Chunk> Default for WholeResponseReplay<Chunk> {
    fn default() -> Self {
        Self {
            next_checkpoint: 0,
            buffered_chunks: VecDeque::new(),
        }
    }
}

impl<Chunk> WholeResponseReplay<Chunk> {
    pub(crate) fn push(&mut self, chunk: Chunk) -> StreamGuardrailDecision {
        let checkpoint = StreamCheckpoint(self.next_checkpoint);
        self.next_checkpoint = self.next_checkpoint.saturating_add(1);
        self.buffered_chunks.push_back((checkpoint, chunk));
        StreamGuardrailDecision::Pending
    }

    pub(crate) fn allow_all(self) -> (StreamGuardrailDecision, VecDeque<Chunk>) {
        let approved_through = self
            .buffered_chunks
            .back()
            .map(|(checkpoint, _)| *checkpoint)
            .unwrap_or(StreamCheckpoint(0));
        let buffered_chunks = self
            .buffered_chunks
            .into_iter()
            .map(|(_, chunk)| chunk)
            .collect();

        (
            StreamGuardrailDecision::Allow { approved_through },
            buffered_chunks,
        )
    }
}

#[derive(Debug, Default)]
pub(crate) struct WholeResponseReplayDriver<Chunk> {
    replay: Option<WholeResponseReplay<Chunk>>,
    replay_queue: VecDeque<Chunk>,
    upstream_finished: bool,
}

impl<Chunk> WholeResponseReplayDriver<Chunk> {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            replay: enabled.then(WholeResponseReplay::default),
            replay_queue: VecDeque::new(),
            upstream_finished: false,
        }
    }

    pub(crate) fn take_replay_chunk(&mut self) -> Option<Chunk> {
        self.replay_queue.pop_front()
    }

    pub(crate) fn finish_upstream(&mut self) -> WholeResponseReplayFinalize {
        if self.replay.is_some() {
            WholeResponseReplayFinalize::NeedsGuardrailCheck
        } else {
            self.upstream_finished = true;
            WholeResponseReplayFinalize::Finished
        }
    }

    pub(crate) fn is_upstream_finished(&self) -> bool {
        self.upstream_finished
    }

    pub(crate) fn is_buffering(&self) -> bool {
        self.replay.is_some()
    }
}

impl<Chunk: Clone> WholeResponseReplayDriver<Chunk> {
    pub(crate) fn push_upstream_chunk(&mut self, chunk: Chunk) -> WholeResponseReplayAction<Chunk> {
        if let Some(replay) = self.replay.as_mut() {
            let decision = replay.push(chunk.clone());
            debug_assert!(matches!(decision, StreamGuardrailDecision::Pending));
            WholeResponseReplayAction::Buffered(chunk)
        } else {
            WholeResponseReplayAction::Emit(chunk)
        }
    }

    pub(crate) fn approve_buffered(&mut self) -> StreamGuardrailDecision {
        let Some(replay) = self.replay.take() else {
            debug_assert!(
                false,
                "approve_buffered called without buffered replay state"
            );
            return StreamGuardrailDecision::Allow {
                approved_through: StreamCheckpoint(0),
            };
        };

        let (decision, drained_chunks) = replay.allow_all();
        self.replay_queue = drained_chunks;
        self.upstream_finished = true;
        decision
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{
        StreamGuardrailDecision, WholeResponseReplayAction, WholeResponseReplayDriver,
        WholeResponseReplayFinalize,
    };

    #[test]
    fn whole_response_replay_driver_passes_through_when_disabled() {
        let mut driver = WholeResponseReplayDriver::new(false);

        assert_eq!(
            driver.push_upstream_chunk(7_u8),
            WholeResponseReplayAction::Emit(7)
        );
        assert!(!driver.is_buffering());
        assert_eq!(
            driver.finish_upstream(),
            WholeResponseReplayFinalize::Finished
        );
        assert!(driver.is_upstream_finished());
        assert_eq!(driver.take_replay_chunk(), None);
    }

    #[test]
    fn whole_response_replay_driver_replays_buffered_chunks_after_approval() {
        let mut driver = WholeResponseReplayDriver::new(true);

        assert_eq!(
            driver.push_upstream_chunk(String::from("safe ")),
            WholeResponseReplayAction::Buffered(String::from("safe ")),
        );
        assert_eq!(
            driver.push_upstream_chunk(String::from("response")),
            WholeResponseReplayAction::Buffered(String::from("response")),
        );
        assert!(driver.is_buffering());
        assert_eq!(
            driver.finish_upstream(),
            WholeResponseReplayFinalize::NeedsGuardrailCheck,
        );
        assert!(matches!(
            driver.approve_buffered(),
            StreamGuardrailDecision::Allow { .. }
        ));
        assert!(!driver.is_buffering());
        assert!(driver.is_upstream_finished());
        assert_eq!(driver.take_replay_chunk(), Some(String::from("safe ")));
        assert_eq!(driver.take_replay_chunk(), Some(String::from("response")));
        assert_eq!(driver.take_replay_chunk(), None);
    }
}
