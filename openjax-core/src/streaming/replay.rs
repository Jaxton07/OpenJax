use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayWindowError {
    pub requested_after_seq: u64,
    pub min_allowed: u64,
}

#[derive(Debug, Clone)]
pub struct ReplayBuffer<T: Clone> {
    capacity: usize,
    next_seq: u64,
    entries: VecDeque<(u64, T)>,
}

impl<T: Clone> ReplayBuffer<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            next_seq: 1,
            entries: VecDeque::new(),
        }
    }

    pub fn push(&mut self, item: T) -> u64 {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);
        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back((seq, item));
        seq
    }

    pub fn replay_from(&self, after_seq: Option<u64>) -> Result<Vec<(u64, T)>, ReplayWindowError> {
        let min_allowed = self
            .entries
            .front()
            .map(|(seq, _)| seq.saturating_sub(1))
            .unwrap_or(0);

        if let Some(requested) = after_seq
            && requested < min_allowed
        {
            return Err(ReplayWindowError {
                requested_after_seq: requested,
                min_allowed,
            });
        }

        Ok(self
            .entries
            .iter()
            .filter(|(seq, _)| after_seq.is_none_or(|after| *seq > after))
            .map(|(seq, item)| (*seq, item.clone()))
            .collect())
    }

    pub fn last_seq(&self) -> u64 {
        self.entries.back().map(|(seq, _)| *seq).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::{ReplayBuffer, ReplayWindowError};

    #[test]
    fn replay_rejects_out_of_window_request() {
        let mut replay = ReplayBuffer::with_capacity(2);
        replay.push("a");
        replay.push("b");
        replay.push("c");

        let result = replay.replay_from(Some(0));
        assert!(matches!(
            result,
            Err(ReplayWindowError {
                requested_after_seq: 0,
                min_allowed: 1
            })
        ));
    }
}
