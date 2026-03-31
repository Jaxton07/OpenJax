use serde::{Deserialize, Serialize};

use crate::tools::handlers::de_helpers;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessSortBy {
    #[default]
    Cpu,
    Memory,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProcessSnapshotArgs {
    #[serde(default)]
    pub sort_by: ProcessSortBy,
    #[serde(
        default = "default_process_limit",
        deserialize_with = "de_helpers::de_usize"
    )]
    pub limit: usize,
    pub user: Option<String>,
}

fn default_process_limit() -> usize {
    10
}

#[derive(Debug, Clone, Deserialize)]
pub struct SystemLoadArgs {
    #[serde(default = "default_true", deserialize_with = "de_helpers::de_bool")]
    pub include_cpu: bool,
    #[serde(default = "default_true", deserialize_with = "de_helpers::de_bool")]
    pub include_memory: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiskUsageArgs {
    pub path: Option<String>,
    #[serde(default, deserialize_with = "de_helpers::de_bool")]
    pub include_all_mounts: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct ProcessRecord {
    pub pid: u32,
    pub name: String,
    pub cpu_pct: f32,
    pub memory_bytes: u64,
    pub memory_pct: f64,
    pub user: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct CpuLoadRecord {
    pub logical_cores: usize,
    pub usage_pct: f32,
}

#[derive(Debug, Clone)]
pub struct MemoryLoadRecord {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub used_pct: f64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct LoadAverageRecord {
    pub one: f64,
    pub five: f64,
    pub fifteen: f64,
}

#[derive(Debug, Clone)]
pub struct DiskUsageRecord {
    pub mount_point: String,
    pub fs_name: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}
