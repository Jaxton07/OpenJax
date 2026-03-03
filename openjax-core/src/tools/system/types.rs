use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessSortBy {
    Cpu,
    Memory,
}

impl Default for ProcessSortBy {
    fn default() -> Self {
        Self::Cpu
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProcessSnapshotArgs {
    #[serde(default)]
    pub sort_by: ProcessSortBy,
    #[serde(
        default = "default_process_limit",
        deserialize_with = "deserialize_usize_or_string"
    )]
    pub limit: usize,
    pub user: Option<String>,
}

fn default_process_limit() -> usize {
    10
}

#[derive(Debug, Clone, Deserialize)]
pub struct SystemLoadArgs {
    #[serde(default = "default_true", deserialize_with = "deserialize_boolish")]
    pub include_cpu: bool,
    #[serde(default = "default_true", deserialize_with = "deserialize_boolish")]
    pub include_memory: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiskUsageArgs {
    pub path: Option<String>,
    #[serde(default, deserialize_with = "deserialize_boolish")]
    pub include_all_mounts: bool,
}

fn default_true() -> bool {
    true
}

fn deserialize_usize_or_string<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    if let Some(num) = value.as_u64() {
        return Ok(num as usize);
    }
    if let Some(text) = value.as_str() {
        return text
            .parse::<usize>()
            .map_err(|_| serde::de::Error::custom("expected positive integer"));
    }
    Err(serde::de::Error::custom("expected integer"))
}

fn deserialize_boolish<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    if let Some(flag) = value.as_bool() {
        return Ok(flag);
    }
    if let Some(text) = value.as_str() {
        return Ok(matches!(
            text.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        ));
    }
    Ok(false)
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
