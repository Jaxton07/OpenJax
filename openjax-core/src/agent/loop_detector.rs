use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopSignal {
    None,
    Warned,
    Halt,
}

#[derive(Clone)]
pub struct LoopDetector {
    window: VecDeque<(String, String)>,
    state: LoopSignal,
    warned_tool: Option<(String, String)>,
    window_capacity: usize,
    warn_threshold: usize,
}

impl LoopDetector {
    pub fn new() -> Self {
        Self {
            window: VecDeque::with_capacity(16),
            state: LoopSignal::None,
            warned_tool: None,
            window_capacity: 16,
            warn_threshold: 5,
        }
    }

    pub fn check_and_advance(&mut self, tool_name: &str, args_hash: &str) -> LoopSignal {
        let key = (tool_name.to_string(), args_hash.to_string());

        if self.state == LoopSignal::Warned {
            if Some(&key) == self.warned_tool.as_ref() {
                self.state = LoopSignal::Halt;
                return LoopSignal::Halt;
            } else {
                self.state = LoopSignal::None;
                self.warned_tool = None;
            }
        }

        self.window.push_back(key.clone());
        if self.window.len() > self.window_capacity {
            self.window.pop_front();
        }

        let consecutive_count = self.window.iter().rev()
            .take_while(|k| *k == &key)
            .count();

        if consecutive_count >= self.warn_threshold {
            self.state = LoopSignal::Warned;
            self.warned_tool = Some(key);
            return LoopSignal::Warned;
        }

        LoopSignal::None
    }

    pub fn recovery_prompt(&self) -> Option<&'static str> {
        if self.state == LoopSignal::Warned {
            Some("[系统警告] 检测到你最近连续多次以完全相同的参数调用了同一工具，这可能是陷入了循环。请评估当前执行策略是否有效，并明确下一步将如何调整（可更换工具、改变参数、或给出阶段性结论）。")
        } else {
            None
        }
    }

    pub fn reset(&mut self) {
        self.window.clear();
        self.state = LoopSignal::None;
        self.warned_tool = None;
    }

    pub fn current_state(&self) -> LoopSignal {
        self.state
    }

    #[allow(dead_code)]
    pub fn warn_threshold(&self) -> usize {
        self.warn_threshold
    }
}

impl Default for LoopDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> LoopDetector {
        LoopDetector::new()
    }

    #[test]
    fn test_normal_calls_return_none() {
        let mut d = fresh();
        assert_eq!(d.check_and_advance("read_file", "hash_a"), LoopSignal::None);
        assert_eq!(d.check_and_advance("write_file", "hash_b"), LoopSignal::None);
        assert_eq!(d.check_and_advance("grep", "hash_c"), LoopSignal::None);
        assert_eq!(d.check_and_advance("read_file", "hash_d"), LoopSignal::None);
        assert_eq!(d.check_and_advance("bash", "hash_e"), LoopSignal::None);
    }

    #[test]
    fn test_five_same_calls_trigger_warned() {
        let mut d = fresh();
        for _ in 0..4 {
            assert_eq!(d.check_and_advance("read_file", "hash_x"), LoopSignal::None);
        }
        assert_eq!(d.check_and_advance("read_file", "hash_x"), LoopSignal::Warned);
        assert_eq!(d.current_state(), LoopSignal::Warned);
    }

    #[test]
    fn test_warned_then_different_tool_resets() {
        let mut d = fresh();
        for _ in 0..5 {
            d.check_and_advance("read_file", "hash_x");
        }
        assert_eq!(d.check_and_advance("write_file", "hash_y"), LoopSignal::None);
        assert_eq!(d.current_state(), LoopSignal::None);
    }

    #[test]
    fn test_warned_then_same_call_halts() {
        let mut d = fresh();
        for _ in 0..5 {
            d.check_and_advance("read_file", "hash_x");
        }
        assert_eq!(d.check_and_advance("read_file", "hash_x"), LoopSignal::Halt);
        assert_eq!(d.current_state(), LoopSignal::Halt);
    }

    #[test]
    fn test_reset_clears_state() {
        let mut d = fresh();
        for _ in 0..5 {
            d.check_and_advance("read_file", "hash_x");
        }
        d.reset();
        assert_eq!(d.current_state(), LoopSignal::None);
        assert!(d.window.is_empty());
    }

    #[test]
    fn test_boundary_five_consecutive() {
        let mut d = fresh();
        // 4 same + 1 other
        for _ in 0..4 {
            assert_eq!(d.check_and_advance("read_file", "hash_x"), LoopSignal::None);
        }
        assert_eq!(d.check_and_advance("bash", "hash_other"), LoopSignal::None);
        // now 5 same again
        for _ in 0..4 {
            assert_eq!(d.check_and_advance("read_file", "hash_x"), LoopSignal::None);
        }
        assert_eq!(d.check_and_advance("read_file", "hash_x"), LoopSignal::Warned);
    }

    #[test]
    fn test_recovery_prompt_when_warned() {
        let mut d = fresh();
        for _ in 0..5 {
            d.check_and_advance("read_file", "hash_x");
        }
        assert!(d.recovery_prompt().is_some());
        let d2 = fresh();
        assert!(d2.recovery_prompt().is_none());
    }
}
