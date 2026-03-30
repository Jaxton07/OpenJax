use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

use super::types::{
    DEFAULT_SEGMENT_MAX_BYTES, TRANSCRIPT_SCHEMA_VERSION, TranscriptManifest, TranscriptRecord,
};

const SESSIONS_DIR: &str = "sessions";
const SEGMENTS_DIR: &str = "segments";
const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, Clone)]
pub struct TranscriptStore {
    root: PathBuf,
    segment_max_bytes: u64,
}

impl TranscriptStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            segment_max_bytes: DEFAULT_SEGMENT_MAX_BYTES,
        }
    }

    pub fn with_segment_max_bytes(root: impl Into<PathBuf>, segment_max_bytes: u64) -> Self {
        Self {
            root: root.into(),
            segment_max_bytes: segment_max_bytes.max(1),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn append(&self, record: &TranscriptRecord) -> Result<TranscriptRecord> {
        let session_root = self.session_root(&record.session_id);
        let segments_root = session_root.join(SEGMENTS_DIR);
        fs::create_dir_all(&segments_root)
            .with_context(|| format!("create transcript dir {}", segments_root.display()))?;

        let manifest_path = session_root.join(MANIFEST_FILE);
        let mut manifest = if manifest_path.exists() {
            self.read_manifest(&manifest_path)?
        } else {
            TranscriptManifest::new_empty(&record.session_id, record.timestamp.clone())
        };

        let _ = self.recover_manifest_from_tail_if_needed(
            &record.session_id,
            &segments_root,
            &manifest_path,
            &mut manifest,
        )?;
        let _ = self.rotate_if_active_segment_unwritable(
            &record.session_id,
            &segments_root,
            &manifest_path,
            &mut manifest,
        )?;

        let mut next_record = record.clone();
        next_record.schema_version = TRANSCRIPT_SCHEMA_VERSION;
        next_record.event_seq = manifest.last_event_seq.saturating_add(1);
        next_record.session_id = manifest.session_id.clone();

        let encoded = serde_json::to_vec(&next_record).context("serialize transcript record")?;
        let encoded_len_with_newline = u64::try_from(encoded.len() + 1).unwrap_or(u64::MAX);
        if self.should_rotate_by_size(&segments_root, &manifest, encoded_len_with_newline)? {
            manifest.active_segment = self.next_segment_name(&manifest.active_segment)?;
        }
        let _ = self.rotate_if_active_segment_unwritable(
            &record.session_id,
            &segments_root,
            &manifest_path,
            &mut manifest,
        )?;

        let segment_path = segments_root.join(&manifest.active_segment);
        let mut segment_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&segment_path)
            .with_context(|| format!("open transcript segment {}", segment_path.display()))?;
        segment_file
            .write_all(&encoded)
            .with_context(|| format!("write transcript record {}", segment_path.display()))?;
        segment_file
            .write_all(b"\n")
            .with_context(|| format!("flush transcript newline {}", segment_path.display()))?;

        manifest.last_event_seq = next_record.event_seq;
        manifest.last_turn_seq = next_record.turn_seq;
        manifest.updated_at = next_record.timestamp.clone();
        self.write_manifest(&manifest_path, &manifest)?;
        Ok(next_record)
    }

    pub fn replay(&self, session_id: &str, after: Option<u64>) -> Result<Vec<TranscriptRecord>> {
        let session_root = self.session_root(session_id);
        let manifest_path = session_root.join(MANIFEST_FILE);
        if !manifest_path.exists() {
            return Ok(Vec::new());
        }
        let manifest = self.read_manifest(&manifest_path)?;
        let segments_root = session_root.join(SEGMENTS_DIR);
        if !segments_root.exists() {
            return Ok(Vec::new());
        }

        let mut segment_names = Vec::new();
        for entry in fs::read_dir(&segments_root)
            .with_context(|| format!("read segments dir {}", segments_root.display()))?
        {
            let entry = entry
                .with_context(|| format!("read segment entry from {}", segments_root.display()))?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            if self.parse_segment_index(&file_name).is_ok() {
                segment_names.push(file_name);
            }
        }
        segment_names.sort_by_key(|name| self.parse_segment_index(name).unwrap_or(0));

        let mut records = Vec::new();
        for name in segment_names {
            let segment_path = segments_root.join(name);
            let file = match fs::File::open(&segment_path) {
                Ok(file) => file,
                Err(err) if err.kind() == ErrorKind::IsADirectory => continue,
                Err(err) => {
                    return Err(err).with_context(|| {
                        format!("open transcript segment {}", segment_path.display())
                    });
                }
            };
            let reader = BufReader::new(file);
            for line in reader.lines() {
                let line = line.with_context(|| {
                    format!("read transcript line from {}", segment_path.display())
                })?;
                if line.trim().is_empty() {
                    continue;
                }
                let record: TranscriptRecord = serde_json::from_str(&line).with_context(|| {
                    format!("parse transcript line in {}", segment_path.display())
                })?;
                if record.session_id != manifest.session_id {
                    continue;
                }
                if after.map(|seq| record.event_seq > seq).unwrap_or(true) {
                    records.push(record);
                }
            }
        }
        Ok(records)
    }

    pub fn gc(&self, retention_days: u32) -> Result<()> {
        let sessions_root = self.root.join(SESSIONS_DIR);
        if !sessions_root.exists() {
            return Ok(());
        }
        let cutoff = OffsetDateTime::now_utc() - Duration::days(i64::from(retention_days));
        for entry in fs::read_dir(&sessions_root)
            .with_context(|| format!("read sessions dir {}", sessions_root.display()))?
        {
            let entry = entry
                .with_context(|| format!("read session entry from {}", sessions_root.display()))?;
            let session_root = entry.path();
            if !session_root.is_dir() {
                continue;
            }
            let manifest_path = session_root.join(MANIFEST_FILE);
            if !manifest_path.exists() {
                continue;
            }
            let manifest = self.read_manifest(&manifest_path)?;
            let updated_at = match OffsetDateTime::parse(&manifest.updated_at, &Rfc3339) {
                Ok(value) => value,
                Err(_) => continue,
            };
            if updated_at < cutoff {
                fs::remove_dir_all(&session_root).with_context(|| {
                    format!(
                        "remove expired transcript session {}",
                        session_root.display()
                    )
                })?;
            }
        }
        Ok(())
    }

    pub fn recover_manifest_from_active_segment_tail(
        &self,
        session_id: &str,
    ) -> Result<Vec<String>> {
        let session_root = self.session_root(session_id);
        let manifest_path = session_root.join(MANIFEST_FILE);
        if !manifest_path.exists() {
            return Ok(Vec::new());
        }
        let segments_root = session_root.join(SEGMENTS_DIR);
        let mut manifest = self.read_manifest(&manifest_path)?;
        self.recover_manifest_from_tail_if_needed(
            session_id,
            &segments_root,
            &manifest_path,
            &mut manifest,
        )
    }

    pub fn rotate_when_active_segment_unwritable(&self, session_id: &str) -> Result<Vec<String>> {
        let session_root = self.session_root(session_id);
        let manifest_path = session_root.join(MANIFEST_FILE);
        if !manifest_path.exists() {
            return Ok(Vec::new());
        }
        let segments_root = session_root.join(SEGMENTS_DIR);
        fs::create_dir_all(&segments_root)
            .with_context(|| format!("create transcript dir {}", segments_root.display()))?;
        let mut manifest = self.read_manifest(&manifest_path)?;
        self.rotate_if_active_segment_unwritable(
            session_id,
            &segments_root,
            &manifest_path,
            &mut manifest,
        )
    }

    fn session_root(&self, session_id: &str) -> PathBuf {
        self.root.join(SESSIONS_DIR).join(session_id)
    }

    fn should_rotate_by_size(
        &self,
        segments_root: &Path,
        manifest: &TranscriptManifest,
        new_record_size: u64,
    ) -> Result<bool> {
        let segment_path = segments_root.join(&manifest.active_segment);
        if !segment_path.exists() {
            return Ok(false);
        }
        let metadata = fs::metadata(&segment_path)
            .with_context(|| format!("read segment metadata {}", segment_path.display()))?;
        if metadata.is_dir() {
            return Ok(false);
        }
        Ok(metadata.len().saturating_add(new_record_size) > self.segment_max_bytes)
    }

    fn recover_manifest_from_tail_if_needed(
        &self,
        session_id: &str,
        segments_root: &Path,
        manifest_path: &Path,
        manifest: &mut TranscriptManifest,
    ) -> Result<Vec<String>> {
        let active_path = segments_root.join(&manifest.active_segment);
        let tail = self.read_segment_tail(&active_path)?;
        let mut warnings = Vec::new();
        if let Some(tail_record) = tail
            && tail_record.event_seq > manifest.last_event_seq
        {
            warnings.push(format!(
                "manifest_tail_recovered session={session_id} manifest_seq={} tail_seq={}",
                manifest.last_event_seq, tail_record.event_seq
            ));
            manifest.last_event_seq = tail_record.event_seq;
            manifest.last_turn_seq = tail_record.turn_seq;
            manifest.updated_at = tail_record.timestamp;
            self.write_manifest(manifest_path, manifest)?;
        }
        Ok(warnings)
    }

    fn rotate_if_active_segment_unwritable(
        &self,
        session_id: &str,
        segments_root: &Path,
        manifest_path: &Path,
        manifest: &mut TranscriptManifest,
    ) -> Result<Vec<String>> {
        let active_path = segments_root.join(&manifest.active_segment);
        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&active_path)
        {
            Ok(_) => Ok(Vec::new()),
            Err(_) => {
                let stale_segment = manifest.active_segment.clone();
                let rotated = self.next_segment_name(&stale_segment)?;
                manifest.active_segment = rotated.clone();
                self.write_manifest(manifest_path, manifest)?;
                let rotated_path = segments_root.join(&manifest.active_segment);
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&rotated_path)
                    .with_context(|| {
                        format!("open rotated transcript segment {}", rotated_path.display())
                    })?;
                Ok(vec![format!(
                    "active_segment_unwritable_rotated session={session_id} from={stale_segment} to={rotated}"
                )])
            }
        }
    }

    fn read_segment_tail(&self, segment_path: &Path) -> Result<Option<TranscriptRecord>> {
        if !segment_path.exists() {
            return Ok(None);
        }
        if segment_path.is_dir() {
            return Ok(None);
        }
        let content = fs::read_to_string(segment_path)
            .with_context(|| format!("read transcript segment {}", segment_path.display()))?;
        let Some(last_line) = content.lines().rev().find(|line| !line.trim().is_empty()) else {
            return Ok(None);
        };
        let record = serde_json::from_str::<TranscriptRecord>(last_line)
            .with_context(|| format!("parse transcript tail line {}", segment_path.display()))?;
        Ok(Some(record))
    }

    fn next_segment_name(&self, current: &str) -> Result<String> {
        let index = self.parse_segment_index(current)?;
        Ok(format!("segment-{index:06}.jsonl", index = index + 1))
    }

    fn parse_segment_index(&self, file_name: &str) -> Result<u64> {
        let prefix = "segment-";
        let suffix = ".jsonl";
        if !(file_name.starts_with(prefix) && file_name.ends_with(suffix)) {
            anyhow::bail!("invalid segment filename: {file_name}");
        }
        let digits = &file_name[prefix.len()..file_name.len() - suffix.len()];
        let index = digits
            .parse::<u64>()
            .with_context(|| format!("parse segment index from {file_name}"))?;
        Ok(index)
    }

    fn read_manifest(&self, manifest_path: &Path) -> Result<TranscriptManifest> {
        let manifest_raw = fs::read_to_string(manifest_path)
            .with_context(|| format!("read transcript manifest {}", manifest_path.display()))?;
        let manifest = serde_json::from_str(&manifest_raw)
            .with_context(|| format!("parse transcript manifest {}", manifest_path.display()))?;
        Ok(manifest)
    }

    fn write_manifest(&self, manifest_path: &Path, manifest: &TranscriptManifest) -> Result<()> {
        let manifest_json =
            serde_json::to_string_pretty(manifest).context("serialize transcript manifest")?;
        fs::write(manifest_path, manifest_json)
            .with_context(|| format!("write transcript manifest {}", manifest_path.display()))?;
        Ok(())
    }
}
