use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub(crate) struct DispatchTiming {
    started_at: Instant,
}

impl DispatchTiming {
    pub(crate) fn started() -> Self {
        Self {
            started_at: Instant::now(),
        }
    }

    pub(crate) fn elapsed_ms(self) -> u64 {
        self.started_at.elapsed().as_millis() as u64
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DispatchMetrics {
    pub(crate) provider_recv_ts_ms: Option<u64>,
    pub(crate) dispatcher_lock_ts_ms: Option<u64>,
    pub(crate) gateway_emit_ts_ms: Option<u64>,
    pub(crate) mistaken_branch_count: u64,
}
