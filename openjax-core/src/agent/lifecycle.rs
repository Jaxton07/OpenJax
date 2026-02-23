use crate::{Agent, ThreadId, tools};

impl Agent {
    /// Get current agent's thread ID
    pub fn thread_id(&self) -> ThreadId {
        self.thread_id
    }

    /// Get parent thread ID (None for root agent)
    pub fn parent_thread_id(&self) -> Option<ThreadId> {
        self.parent_thread_id
    }

    /// Get agent depth in the hierarchy
    pub fn depth(&self) -> i32 {
        self.depth
    }

    /// Check if this agent can spawn sub-agents
    pub fn can_spawn_sub_agent(&self) -> bool {
        self.depth < tools::MAX_AGENT_DEPTH
    }

    /// Create a new sub-agent (预留扩展，未完全实现)
    /// Returns a new Agent instance with incremented depth
    pub fn spawn_sub_agent(&self, _input: &str) -> Result<Agent, String> {
        if !self.can_spawn_sub_agent() {
            return Err(format!(
                "cannot spawn sub-agent: max depth {} reached",
                tools::MAX_AGENT_DEPTH
            ));
        }

        let mut sub_agent = Agent::with_runtime(
            self.tool_runtime_config.approval_policy,
            self.tool_runtime_config.sandbox_mode,
            self.cwd.clone(),
        );

        // Set parent relationship
        sub_agent.parent_thread_id = Some(self.thread_id);
        sub_agent.depth = self.depth + 1;
        sub_agent.approval_handler = self.approval_handler.clone();

        Ok(sub_agent)
    }
}
