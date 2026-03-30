use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
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
const MANIFEST_FILE: &str = "manifest.json";
const COMPACT_MAX_LINES: usize = 1000;
const COMPACT_MAX_BYTES: u64 = 4 * 1024 * 1024;
const STAGING_STALE_SECS: u64 = 10 * 60;

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
    test_fail_compact_publish: bool,
    test_fail_compact_rollback: bool,
    append_attempts: AtomicUsize,
}

impl SessionIndexStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        Self::new_with_test_failures(root, None, false, false)
    }

    pub fn new_with_test_fail_append_nth(
        root: impl Into<PathBuf>,
        test_fail_append_nth: Option<usize>,
    ) -> Result<Self> {
        Self::new_with_test_failures(root, test_fail_append_nth, false, false)
    }

    pub fn new_with_test_failures(
        root: impl Into<PathBuf>,
        test_fail_append_nth: Option<usize>,
        test_fail_compact_publish: bool,
        test_fail_compact_rollback: bool,
    ) -> Result<Self> {
        let root = root.into();
        let entries = load_with_recovery_or_fail(&root)?;
        Ok(Self {
            root,
            entries: StdMutex::new(entries),
            health: StdMutex::new(IndexHealth::Healthy),
            write_lock: Mutex::new(()),
            test_fail_append_nth,
            test_fail_compact_publish,
            test_fail_compact_rollback,
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

        self.maybe_compact()?;
        Ok(())
    }

    pub async fn upsert_session_index_entry(&self, entry: IndexSessionEntry) -> Result<()> {
        let _guard = self.write_lock.lock().await;
        self.ensure_writable()?;
        let op = SessionIndexLogRecord {
            op: IndexLogOpKind::UpsertSession,
            session_id: entry.session_id.clone(),
            ts: now_rfc3339()?,
            payload: Some(entry.clone()),
        };
        self.append_log_record(&op)?;
        self.upsert_memory_entry(entry);
        self.maybe_compact()?;
        Ok(())
    }

    pub async fn touch_session_index_entry(
        &self,
        session_id: &str,
        updated_at: String,
        last_event_seq: u64,
        last_preview: String,
    ) -> Result<()> {
        let _guard = self.write_lock.lock().await;
        self.ensure_writable()?;
        let base = self
            .entries_guard()
            .iter()
            .find(|entry| entry.session_id == session_id)
            .cloned()
            .with_context(|| format!("session not found for touch: {session_id}"))?;
        let next = IndexSessionEntry {
            session_id: base.session_id.clone(),
            title: base.title.clone(),
            created_at: base.created_at.clone(),
            updated_at,
            last_event_seq,
            last_preview,
        };
        let op = SessionIndexLogRecord {
            op: IndexLogOpKind::UpsertSession,
            session_id: next.session_id.clone(),
            ts: now_rfc3339()?,
            payload: Some(next.clone()),
        };
        self.append_log_record(&op)?;
        self.upsert_memory_entry(next);
        self.maybe_compact()?;
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

        self.maybe_compact()?;
        Ok(())
    }

    fn maybe_compact(&self) -> Result<()> {
        let sessions_root = self.sessions_root();
        let log_path = sessions_root.join(INDEX_LOG_FILE);
        if !log_path.exists() {
            return Ok(());
        }
        let bytes = fs::metadata(&log_path)
            .with_context(|| format!("read log metadata {}", log_path.display()))?
            .len();
        let lines = count_log_lines(&log_path)?;
        if bytes < COMPACT_MAX_BYTES && lines < COMPACT_MAX_LINES {
            return Ok(());
        }

        let entries = self.entries_guard().clone();
        let snapshot = SessionIndexSnapshot {
            schema_version: SESSION_INDEX_SCHEMA_VERSION,
            updated_at: now_rfc3339()?,
            sessions: entries,
        };
        write_json_atomic(&sessions_root.join(INDEX_SNAPSHOT_FILE), &snapshot)?;
        if let Err(err) = rotate_log_tmp_bak(
            &sessions_root,
            self.test_fail_compact_publish,
            self.test_fail_compact_rollback,
        ) {
            if format!("{err:#}").contains("index_repair_required") {
                self.mark_repair_required();
                return Err(err).context("index_repair_required");
            }
            return Err(err);
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

        append_log_record_to_path(&self.sessions_root().join(INDEX_LOG_FILE), record)
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

#[derive(Debug, Serialize, Deserialize)]
struct SessionMetadata {
    schema_version: u32,
    session_id: String,
    title: Option<String>,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ManifestFile {
    #[serde(default)]
    last_event_seq: u64,
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

fn load_with_recovery_or_fail(root: &Path) -> Result<Vec<IndexSessionEntry>> {
    let sessions_root = root.join(SESSIONS_DIR);
    fs::create_dir_all(&sessions_root)
        .with_context(|| format!("create sessions root {}", sessions_root.display()))?;
    cleanup_staging_dirs(&sessions_root)?;

    let mut entries_by_session = match load_entries_snapshot_and_log(&sessions_root) {
        Ok(entries) => entries,
        Err(load_err) => rebuild_from_sessions_dir(&sessions_root).with_context(|| {
            format!("rebuild_from_sessions_dir after load failure: {load_err:#}")
        })?,
    };

    startup_audit(&sessions_root, &mut entries_by_session)?;
    let mut entries: Vec<IndexSessionEntry> = entries_by_session.into_values().collect();
    sort_entries_desc(&mut entries);
    Ok(entries)
}

fn load_entries_snapshot_and_log(sessions_root: &Path) -> Result<HashMap<String, IndexSessionEntry>> {
    let mut entries_by_session = load_snapshot(sessions_root)?;
    replay_log(sessions_root, &mut entries_by_session)?;
    Ok(entries_by_session)
}

fn cleanup_staging_dirs(sessions_root: &Path) -> Result<()> {
    let staging_root = sessions_root.join(STAGING_DIR);
    if !staging_root.exists() {
        return Ok(());
    }
    let now = std::time::SystemTime::now();
    for entry in fs::read_dir(&staging_root)
        .with_context(|| format!("read staging dir {}", staging_root.display()))?
    {
        let entry = entry.with_context(|| format!("read entry in {}", staging_root.display()))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let modified = entry
            .metadata()
            .with_context(|| format!("read metadata {}", path.display()))?
            .modified()
            .with_context(|| format!("read modified time {}", path.display()))?;
        let age = now.duration_since(modified).unwrap_or(Duration::from_secs(0));
        if age.as_secs() > STAGING_STALE_SECS {
            fs::remove_dir_all(&path)
                .with_context(|| format!("remove stale staging dir {}", path.display()))?;
        }
    }
    Ok(())
}

fn startup_audit(
    sessions_root: &Path,
    entries_by_session: &mut HashMap<String, IndexSessionEntry>,
) -> Result<()> {
    let mut on_disk = scan_sessions_metadata(sessions_root)?;
    let indexed_ids: Vec<String> = entries_by_session.keys().cloned().collect();
    for session_id in indexed_ids {
        if !on_disk.contains_key(&session_id) {
            entries_by_session.remove(&session_id);
            append_log_record_to_path(
                &sessions_root.join(INDEX_LOG_FILE),
                &SessionIndexLogRecord {
                    op: IndexLogOpKind::DeleteSession,
                    session_id,
                    ts: now_rfc3339()?,
                    payload: None,
                },
            )?;
        }
    }

    for (session_id, entry) in on_disk.drain() {
        if !entries_by_session.contains_key(&session_id) {
            entries_by_session.insert(session_id.clone(), entry.clone());
            append_log_record_to_path(
                &sessions_root.join(INDEX_LOG_FILE),
                &SessionIndexLogRecord {
                    op: IndexLogOpKind::UpsertSession,
                    session_id,
                    ts: now_rfc3339()?,
                    payload: Some(entry),
                },
            )?;
        }
    }
    Ok(())
}

fn rebuild_from_sessions_dir(sessions_root: &Path) -> Result<HashMap<String, IndexSessionEntry>> {
    let entries_by_session = scan_sessions_metadata(sessions_root)?;
    let snapshot = SessionIndexSnapshot {
        schema_version: SESSION_INDEX_SCHEMA_VERSION,
        updated_at: now_rfc3339()?,
        sessions: entries_by_session.values().cloned().collect(),
    };
    write_json_atomic(&sessions_root.join(INDEX_SNAPSHOT_FILE), &snapshot)?;
    rotate_log_tmp_bak(sessions_root, false, false)?;
    Ok(entries_by_session)
}

fn scan_sessions_metadata(sessions_root: &Path) -> Result<HashMap<String, IndexSessionEntry>> {
    let mut out = HashMap::new();
    for entry in fs::read_dir(sessions_root)
        .with_context(|| format!("read sessions dir {}", sessions_root.display()))?
    {
        let entry = entry.with_context(|| format!("read entry in {}", sessions_root.display()))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == STAGING_DIR {
            continue;
        }
        let session_path = path.join(SESSION_METADATA_FILE);
        if !session_path.exists() {
            continue;
        }
        let raw = fs::read_to_string(&session_path)
            .with_context(|| format!("read session metadata {}", session_path.display()))?;
        let metadata: SessionMetadata = serde_json::from_str(&raw)
            .with_context(|| format!("parse session metadata {}", session_path.display()))?;

        let manifest_path = path.join(MANIFEST_FILE);
        let last_event_seq = read_last_event_seq(&manifest_path)?;
        let entry = IndexSessionEntry {
            session_id: metadata.session_id.clone(),
            title: metadata.title,
            created_at: metadata.created_at,
            updated_at: metadata.updated_at,
            last_event_seq,
            last_preview: String::new(),
        };
        out.insert(metadata.session_id, entry);
    }
    Ok(out)
}

fn read_last_event_seq(manifest_path: &Path) -> Result<u64> {
    if !manifest_path.exists() {
        return Ok(0);
    }
    let raw = fs::read_to_string(manifest_path)
        .with_context(|| format!("read manifest {}", manifest_path.display()))?;
    let manifest: ManifestFile = serde_json::from_str(&raw)
        .with_context(|| format!("parse manifest {}", manifest_path.display()))?;
    Ok(manifest.last_event_seq)
}

fn rotate_log_tmp_bak(
    sessions_root: &Path,
    fail_publish: bool,
    fail_rollback: bool,
) -> Result<()> {
    let log_path = sessions_root.join(INDEX_LOG_FILE);
    let tmp_path = sessions_root.join(format!("{INDEX_LOG_FILE}.tmp"));
    let bak_path = sessions_root.join(format!("{INDEX_LOG_FILE}.bak"));

    let mut tmp = File::create(&tmp_path)
        .with_context(|| format!("create compact tmp log {}", tmp_path.display()))?;
    tmp.flush()
        .with_context(|| format!("flush compact tmp log {}", tmp_path.display()))?;
    tmp.sync_all()
        .with_context(|| format!("sync compact tmp log {}", tmp_path.display()))?;

    if log_path.exists() {
        if bak_path.exists() {
            fs::remove_file(&bak_path)
                .with_context(|| format!("remove stale compact bak {}", bak_path.display()))?;
        }
        fs::rename(&log_path, &bak_path).with_context(|| {
            format!(
                "rename live log {} to bak {}",
                log_path.display(),
                bak_path.display()
            )
        })?;
    }

    let publish_result: Result<()> = if fail_publish {
        Err(anyhow::anyhow!("forced compact publish failure"))
    } else {
        fs::rename(&tmp_path, &log_path).with_context(|| {
            format!(
                "rename compact tmp {} to live {}",
                tmp_path.display(),
                log_path.display()
            )
        })
    };
    if let Err(publish_err) = publish_result {
        let rollback_result: Result<()> = if fail_rollback {
            Err(anyhow::anyhow!("forced compact rollback failure"))
        } else if bak_path.exists() {
            fs::rename(&bak_path, &log_path).with_context(|| {
                format!(
                    "rollback compact bak {} to live {}",
                    bak_path.display(),
                    log_path.display()
                )
            })
        } else {
            Ok(())
        };
        let _ = fs::remove_file(&tmp_path);
        if rollback_result.is_err() {
            return Err(publish_err)
                .context("index_repair_required: compact rollback failed after publish failure");
        }
        return Err(publish_err).context("compact publish failed but rollback succeeded");
    }

    sync_parent_dir(&log_path)?;
    if bak_path.exists() {
        fs::remove_file(&bak_path)
            .with_context(|| format!("remove compact bak {}", bak_path.display()))?;
    }
    Ok(())
}

fn append_log_record_to_path(log_path: &Path, record: &SessionIndexLogRecord) -> Result<()> {
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create log dir {}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
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

fn count_log_lines(log_path: &Path) -> Result<usize> {
    let file =
        File::open(log_path).with_context(|| format!("open index log {}", log_path.display()))?;
    let reader = BufReader::new(file);
    let mut count = 0usize;
    for line in reader.lines() {
        let line = line.with_context(|| format!("read index log line {}", log_path.display()))?;
        if !line.trim().is_empty() {
            count = count.saturating_add(1);
        }
    }
    Ok(count)
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
    if snapshot.schema_version != SESSION_INDEX_SCHEMA_VERSION {
        anyhow::bail!(
            "invalid index snapshot schema_version {} expected {} in {}",
            snapshot.schema_version,
            SESSION_INDEX_SCHEMA_VERSION,
            snapshot_path.display()
        );
    }

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
