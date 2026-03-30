use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use anyhow::{Context, Result};
use tokio::sync::Mutex;

use super::session_index_types::{
    INDEX_LOG_FILE, INDEX_SNAPSHOT_FILE, IndexLogOpKind, IndexSessionEntry, SessionIndexLogRecord,
    SessionIndexSnapshot,
};

const SESSIONS_DIR: &str = "sessions";

#[derive(Debug)]
pub struct SessionIndexStore {
    root: PathBuf,
    entries: Vec<IndexSessionEntry>,
    write_lock: Mutex<()>,
}

impl SessionIndexStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let entries = load_entries_from_disk(&root)?;
        Ok(Self {
            root,
            entries,
            write_lock: Mutex::new(()),
        })
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    pub fn list_sessions(&self) -> Vec<IndexSessionEntry> {
        self.entries.clone()
    }

    pub fn write_lock(&self) -> &Mutex<()> {
        &self.write_lock
    }
}

fn load_entries_from_disk(root: &std::path::Path) -> Result<Vec<IndexSessionEntry>> {
    let sessions_root = root.join(SESSIONS_DIR);
    fs::create_dir_all(&sessions_root)
        .with_context(|| format!("create sessions root {}", sessions_root.display()))?;

    let mut entries_by_session = load_snapshot(&sessions_root)?;
    replay_log(&sessions_root, &mut entries_by_session)?;

    let mut entries: Vec<IndexSessionEntry> = entries_by_session.into_values().collect();
    entries.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| right.session_id.cmp(&left.session_id))
    });
    Ok(entries)
}

fn load_snapshot(sessions_root: &std::path::Path) -> Result<HashMap<String, IndexSessionEntry>> {
    let snapshot_path = sessions_root.join(INDEX_SNAPSHOT_FILE);
    if !snapshot_path.exists() {
        return Ok(HashMap::new());
    }

    let raw = fs::read_to_string(&snapshot_path)
        .with_context(|| format!("read index snapshot {}", snapshot_path.display()))?;
    let snapshot: SessionIndexSnapshot = serde_json::from_str(&raw)
        .with_context(|| format!("parse index snapshot {}", snapshot_path.display()))?;

    Ok(snapshot
        .sessions
        .into_iter()
        .map(|entry| (entry.session_id.clone(), entry))
        .collect())
}

fn replay_log(
    sessions_root: &std::path::Path,
    entries_by_session: &mut HashMap<String, IndexSessionEntry>,
) -> Result<()> {
    let log_path = sessions_root.join(INDEX_LOG_FILE);
    if !log_path.exists() {
        return Ok(());
    }

    let file = File::open(&log_path)
        .with_context(|| format!("open index log {}", log_path.display()))?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line.with_context(|| format!("read index log line {}", log_path.display()))?;
        if line.trim().is_empty() {
            continue;
        }
        let record: SessionIndexLogRecord = serde_json::from_str(&line)
            .with_context(|| format!("parse index log line {}", log_path.display()))?;
        match record.op {
            IndexLogOpKind::UpsertSession => {
                let payload = record.payload.with_context(|| {
                    format!("missing upsert payload in index log {}", log_path.display())
                })?;
                entries_by_session.insert(payload.session_id.clone(), payload);
            }
            IndexLogOpKind::DeleteSession => {
                entries_by_session.remove(&record.session_id);
            }
        }
    }

    Ok(())
}
