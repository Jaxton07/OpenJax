use std::path::Path;

use sysinfo::{Disks, System, Users};

use super::errors::SystemToolError;
use super::types::{
    CpuLoadRecord, DiskUsageRecord, LoadAverageRecord, MemoryLoadRecord, ProcessRecord,
};

pub trait SystemMetricsProvider: Send + Sync {
    fn collect_processes(&self) -> Result<Vec<ProcessRecord>, SystemToolError>;
    fn collect_cpu_load(&self) -> Result<CpuLoadRecord, SystemToolError>;
    fn collect_memory_load(&self) -> Result<MemoryLoadRecord, SystemToolError>;
    fn collect_load_average(&self) -> Result<LoadAverageRecord, SystemToolError>;
    fn collect_disks(&self) -> Result<Vec<DiskUsageRecord>, SystemToolError>;
    fn hostname(&self) -> Option<String>;
    fn default_path(&self) -> Result<String, SystemToolError>;
}

#[derive(Debug, Default)]
pub struct DefaultSystemMetricsProvider;

impl DefaultSystemMetricsProvider {
    fn fresh_system() -> System {
        let mut system = System::new_all();
        system.refresh_all();
        system
    }
}

impl SystemMetricsProvider for DefaultSystemMetricsProvider {
    fn collect_processes(&self) -> Result<Vec<ProcessRecord>, SystemToolError> {
        let system = Self::fresh_system();
        let users = Users::new_with_refreshed_list();
        let total_memory = system.total_memory() as f64;

        let processes = system
            .processes()
            .values()
            .map(|process| {
                let memory_bytes = process.memory();
                let memory_pct = if total_memory > 0.0 {
                    (memory_bytes as f64 / total_memory) * 100.0
                } else {
                    0.0
                };
                let user = process
                    .user_id()
                    .and_then(|id| users.get_user_by_id(id))
                    .map(|u| u.name().to_string());

                ProcessRecord {
                    pid: process.pid().as_u32(),
                    name: process.name().to_string_lossy().to_string(),
                    cpu_pct: process.cpu_usage(),
                    memory_bytes,
                    memory_pct,
                    user,
                    status: format!("{:?}", process.status()).to_ascii_lowercase(),
                }
            })
            .collect();

        Ok(processes)
    }

    fn collect_cpu_load(&self) -> Result<CpuLoadRecord, SystemToolError> {
        let system = Self::fresh_system();
        Ok(CpuLoadRecord {
            logical_cores: system.cpus().len(),
            usage_pct: system.global_cpu_usage(),
        })
    }

    fn collect_memory_load(&self) -> Result<MemoryLoadRecord, SystemToolError> {
        let system = Self::fresh_system();
        let total_bytes = system.total_memory();
        let used_bytes = system.used_memory();
        let used_pct = if total_bytes > 0 {
            (used_bytes as f64 / total_bytes as f64) * 100.0
        } else {
            0.0
        };

        Ok(MemoryLoadRecord {
            total_bytes,
            used_bytes,
            used_pct,
            swap_total_bytes: system.total_swap(),
            swap_used_bytes: system.used_swap(),
        })
    }

    fn collect_load_average(&self) -> Result<LoadAverageRecord, SystemToolError> {
        let load = System::load_average();
        Ok(LoadAverageRecord {
            one: load.one,
            five: load.five,
            fifteen: load.fifteen,
        })
    }

    fn collect_disks(&self) -> Result<Vec<DiskUsageRecord>, SystemToolError> {
        let disks = Disks::new_with_refreshed_list();
        Ok(disks
            .iter()
            .map(|disk| DiskUsageRecord {
                mount_point: disk.mount_point().display().to_string(),
                fs_name: disk.file_system().to_string_lossy().to_string(),
                total_bytes: disk.total_space(),
                available_bytes: disk.available_space(),
            })
            .collect())
    }

    fn hostname(&self) -> Option<String> {
        System::host_name()
    }

    fn default_path(&self) -> Result<String, SystemToolError> {
        let path = std::env::current_dir().map_err(|e| {
            SystemToolError::CollectionFailed(format!("failed to resolve current_dir: {e}"))
        })?;
        Ok(path.display().to_string())
    }
}

pub fn choose_disk_by_path<'a>(
    disks: &'a [DiskUsageRecord],
    selected_path: &Path,
) -> Option<&'a DiskUsageRecord> {
    let selected = selected_path.to_string_lossy();
    disks
        .iter()
        .filter(|disk| selected.starts_with(&disk.mount_point))
        .max_by_key(|disk| disk.mount_point.len())
}

#[cfg(test)]
mod tests {
    use super::choose_disk_by_path;
    use crate::tools::system::types::DiskUsageRecord;
    use std::path::Path;

    #[test]
    fn chooses_longest_matching_mount() {
        let disks = vec![
            DiskUsageRecord {
                mount_point: "/".to_string(),
                fs_name: "root".to_string(),
                total_bytes: 100,
                available_bytes: 50,
            },
            DiskUsageRecord {
                mount_point: "/tmp".to_string(),
                fs_name: "tmp".to_string(),
                total_bytes: 80,
                available_bytes: 30,
            },
        ];

        let selected = choose_disk_by_path(&disks, Path::new("/tmp/openjax/test.txt"))
            .expect("should select mount");
        assert_eq!(selected.mount_point, "/tmp");
    }
}
