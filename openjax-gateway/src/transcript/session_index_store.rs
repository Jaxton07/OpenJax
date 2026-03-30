use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};
use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::sync::Mutex;

use super::session_index_types::{
    INDEX_LOG_FILE, INDEX_SNAPSHOT_FILE, IndexLogOpKind, IndexSessionEntry,
    SESSION_INDEX_SCHEMA_VERSION, SessionIndexLogRecord, SessionIndexSnapshot,
};

const SESSIONS_DIR: &str = "sessions";
const STAGING_DIR: &str = ".staging";
const SESSION_METADATA_FILE: &str = "session.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexHealth {
    Healthy,
    RepairRequired,
}

#[derive(Debug)]
pub struct SessionIndexStore {
    root: PathBuf,
    entries: StdMutex<Vec<IndexSessionEntry>>,
    health: StdMutex<IndexHealth>,
    write_lock: Mutex<()>,
    test_fail_append_nth: Option<usize>,
    append_attempts: AtomicUsize,
}

impl SessionIndexStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        Self::new_with_test_fail_append_nth(root, None)
    }

    pub fn new_with_test_fail_append_nth(
        root: impl Into<PathBuf>,
        test_fail_append_nth: Option<usize>,
    ) -> Result<Self> {
        let root = root.into();
        let entries = load_entries_from_disk(&root)?;
        Ok(Self {
            root,
            entries: StdMutex::new(entries),
            health: StdMutex::new(IndexHealth::Healthy),
            write_lock: Mutex::new(()),
            test_fail_append_nth,
            append_attempts: AtomicUsize::new(0),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn list_sessions(&self) -> Vec<IndexSessionEntry> {
        self.entries_guard().clone()
    }

    pub fn write_lock(&self) -> &Mutex<()> {
        &self.write_lock
    }

    pub fn is_repair_required(&self) -> bool {
        self.index_health() == IndexHealth::RepairRequired
    }

    pub fn index_health(&self) -> IndexHealth {
        *self.health_guard()
    }

    pub async fn create_session_index_entry(&self, entry: IndexSessionEntry) -> Result<()> {
        let _guard = self.write_lock.lock().await;
        self.ensure_writable()?;

        let sessions_root = self.sessions_root();
        let staging_dir = sessions_root.join(STAGING_DIR).join(&entry.session_id);
        let published_dir = sessions_root.join(&entry.session_id);
        if staging_dir.exists() {
            fs::remove_dir_all(&staging_dir)
                .with_context(|| format!("remove stale staging dir {}", staging_dir.display()))?;
        }
        fs::create_dir_all(&staging_dir)
            .with_context(|| format!("create staging dir {}", staging_dir.display()))?;

        let metadata = SessionMetadata::from_index_entry(&entry);
        write_json_atomic(&staging_dir.join(SESSION_METADATA_FILE), &metadata)?;

        let upsert = SessionIndexLogRecord {
            op: IndexLogOpKind::UpsertSession,
            session_id: entry.session_id.clone(),
            ts: now_rfc3339()?,
            payload: Some(entry.clone()),
        };
        self.append_log_record(&upsert)?;
        self.upsert_memory_entry(entry.clone());

        if let Err(publish_err) = fs::rename(&staging_dir, &published_dir).with_context(|| {
            format!(
                "publish staged session {} to {}",
                staging_dir.display(),
                published_dir.display()
            )
        }) {
            let compensation = SessionIndexLogRecord {
                op: IndexLogOpKind::DeleteSession,
                session_id: entry.session_id.clone(),
                ts: now_rfc3339()?,
                payload: None,
            };
            if let Err(compensation_err) = self.append_log_record(&compensation) {
                self.mark_repair_required();
                return Err(compensation_err).context(
                    "index_repair_required: compensation append failed after create publish failure",
                );
            }
            self.remove_memory_entry(&entry.session_id);
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(publish_err).context("create session index publish failed and rolled back");
        }

        Ok(())
    }

    pub async fn delete_session_index_entry(&self, session_id: &str) -> Result<()> {
        let _guard = self.write_lock.lock().await;
        self.ensure_writable()?;

        let original = self
            .entries_guard()
            .iter()
            .find(|entry| entry.session_id == session_id)
            .cloned();

        let delete = SessionIndexLogRecord {
            op: IndexLogOpKind::DeleteSession,
            session_id: session_id.to_string(),
            ts: now_rfc3339()?,
            payload: None,
        };
        self.append_log_record(&delete)?;
        self.remove_memory_entry(session_id);

        let session_dir = self.sessions_root().join(session_id);
        if let Err(remove_err) = fs::remove_dir_all(&session_dir)
            .with_context(|| format!("remove session directory {}", session_dir.display()))
        {
            if let Some(entry) = original {
                let compensation = SessionIndexLogRecord {
                    op: IndexLogOpKind::UpsertSession,
                    session_id: entry.session_id.clone(),
                    ts: now_rfc3339()?,
                    payload: Some(entry.clone()),
                };
                if let Err(compensation_err) = self.append_log_record(&compensation) {
                    self.mark_repair_required();
                    return Err(compensation_err).context(
                        "index_repair_required: compensation append failed after delete remove failure",
                    );
                }
                self.upsert_memory_entry(entry);
            }
            return Err(remove_err).context("delete session index remove failed and rolled back");
        }

        Ok(())
    }

    fn append_log_record(&self, record: &SessionIndexLogRecord) -> Result<()> {
        let attempt = self.append_attempts.fetch_add(1, Ordering::SeqCst) + 1;
        if self
            .test_fail_append_nth
            .is_some_and(|expected| expected == attempt)
        {
            anyhow::bail!("forced append log failure at attempt {attempt}");
        }

        let log_path = self.sessions_root().join(INDEX_LOG_FILE);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .with_context(|| format!("open index log {}", log_path.display()))?;
        let encoded = serde_json::to_string(record).context("serialize index log record")?;
        file.write_all(encoded.as_bytes())
            .with_context(|| format!("write index log {}", log_path.display()))?;
        file.write_all(b"\n")
            .with_context(|| format!("write index log newline {}", log_path.display()))?;
        file.flush()
            .with_context(|| format!("flush index log {}", log_path.display()))?;
        file.sync_all()
            .with_context(|| format!("sync index log {}", log_path.display()))?;
        Ok(())
    }

    fn ensure_writable(&self) -> Result<()> {
        if self.is_repair_required() {
            anyhow::bail!("index_repair_required");
        }
        Ok(())
    }

    fn mark_repair_required(&self) {
        *self.health_guard() = IndexHealth::RepairRequired;
    }

    fn upsert_memory_entry(&self, entry: IndexSessionEntry) {
        let mut entries = self.entries_guard();
        if let Some(existing) = entries
            .iter_mut()
            .find(|existing| existing.session_id == entry.session_id)
        {
            *existing = entry;
        } else {
            entries.push(entry);
        }
        sort_entries_desc(&mut entries);
    }

    fn remove_memory_entry(&self, session_id: &str) {
        let mut entries = self.entries_guard();
        entries.retain(|entry| entry.session_id != session_id);
        sort_entries_desc(&mut entries);
    }

    fn sessions_root(&self) -> PathBuf {
        self.root.join(SESSIONS_DIR)
    }

    fn entries_guard(&self) -> std::sync::MutexGuard<'_, Vec<IndexSessionEntry>> {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn health_guard(&self) -> std::sync::MutexGuard<'_, IndexHealth> {
        self.health
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Debug, Serialize)]
struct SessionMetadata {
    schema_version: u32,
    session_id: String,
    title: Option<String>,
    created_at: String,
    updated_at: String,
    tags: Vec<String>,
}

impl SessionMetadata {
    fn from_index_entry(entry: &IndexSessionEntry) -> Self {
        Self {
            schema_version: SESSION_INDEX_SCHEMA_VERSION,
            session_id: entry.session_id.clone(),
            title: entry.title.clone(),
            created_at: entry.created_at.clone(),
            updated_at: entry.updated_at.clone(),
            tags: Vec::new(),
        }
    }
}

fn sort_entries_desc(entries: &mut [IndexSessionEntry]) {
    entries.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| right.session_id.cmp(&left.session_id))
    });
}

fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .context("format current timestamp as rfc3339")
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("resolve parent directory for {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("create parent {}", parent.display()))?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("resolve filename for {}", path.display()))?;
    let tmp_path = parent.join(format!("{file_name}.tmp"));

    let encoded = serde_json::to_vec_pretty(value).context("serialize json payload")?;
    let mut tmp_file = File::create(&tmp_path)
        .with_context(|| format!("create tmp file {}", tmp_path.display()))?;
    tmp_file
        .write_all(&encoded)
        .with_context(|| format!("write tmp file {}", tmp_path.display()))?;
    tmp_file
        .flush()
        .with_context(|| format!("flush tmp file {}", tmp_path.display()))?;
    tmp_file
        .sync_all()
        .with_context(|| format!("sync tmp file {}", tmp_path.display()))?;

    fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "rename tmp file {} to {}",
            tmp_path.display(),
            path.display()
        )
    })?;
    sync_parent_dir(path)?;
    Ok(())
}

fn sync_parent_dir(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("resolve parent dir for {}", path.display()))?;
    let parent_file =
        File::open(parent).with_context(|| format!("open parent dir {}", parent.display()))?;
    parent_file
        .sync_all()
        .with_context(|| format!("sync parent dir {}", parent.display()))?;
    Ok(())
}

fn load_entries_from_disk(root: &Path) -> Result<Vec<IndexSessionEntry>> {
    let sessions_root = root.join(SESSIONS_DIR);
    fs::create_dir_all(&sessions_root)
        .with_context(|| format!("create sessions root {}", sessions_root.display()))?;

    let mut entries_by_session = load_snapshot(&sessions_root)?;
    replay_log(&sessions_root, &mut entries_by_session)?;

    let mut entries: Vec<IndexSessionEntry> = entries_by_session.into_values().collect();
    sort_entries_desc(&mut entries);
    Ok(entries)
}

fn load_snapshot(sessions_root: &Path) -> Result<HashMap<String, IndexSessionEntry>> {
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
    sessions_root: &Path,
    entries_by_session: &mut HashMap<String, IndexSessionEntry>,
) -> Result<()> {
    let log_path = sessions_root.join(INDEX_LOG_FILE);
    if !log_path.exists() {
        return Ok(());
    }

    let file =
        File::open(&log_path).with_context(|| format!("open index log {}", log_path.display()))?;
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
