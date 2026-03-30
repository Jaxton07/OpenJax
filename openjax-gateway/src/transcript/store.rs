use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::types::TranscriptManifest;
use super::types::TranscriptRecord;

const SESSIONS_DIR: &str = "sessions";
const SEGMENTS_DIR: &str = "segments";
const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, Clone)]
pub struct TranscriptStore {
    root: PathBuf,
}

impl TranscriptStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn append(&self, record: &TranscriptRecord) -> Result<()> {
        let session_root = self.session_root(&record.session_id);
        let segments_root = session_root.join(SEGMENTS_DIR);
        fs::create_dir_all(&segments_root)
            .with_context(|| format!("create transcript dir {}", segments_root.display()))?;

        let manifest_path = session_root.join(MANIFEST_FILE);
        let mut manifest = if manifest_path.exists() {
            self.read_manifest(&manifest_path)?
        } else {
            TranscriptManifest::new_for_first_record(record)
        };

        let segment_path = segments_root.join(&manifest.active_segment);
        let mut segment_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&segment_path)
            .with_context(|| format!("open transcript segment {}", segment_path.display()))?;
        serde_json::to_writer(&mut segment_file, record)
            .with_context(|| format!("write transcript record {}", segment_path.display()))?;
        segment_file
            .write_all(b"\n")
            .with_context(|| format!("flush transcript newline {}", segment_path.display()))?;

        manifest.last_event_seq = record.event_seq;
        manifest.last_turn_seq = record.turn_seq;
        manifest.updated_at = record.timestamp.clone();
        self.write_manifest(&manifest_path, &manifest)?;
        Ok(())
    }

    fn session_root(&self, session_id: &str) -> PathBuf {
        self.root.join(SESSIONS_DIR).join(session_id)
    }

    fn read_manifest(&self, manifest_path: &Path) -> Result<TranscriptManifest> {
        let manifest_raw = fs::read_to_string(manifest_path)
            .with_context(|| format!("read transcript manifest {}", manifest_path.display()))?;
        let manifest = serde_json::from_str(&manifest_raw)
            .with_context(|| format!("parse transcript manifest {}", manifest_path.display()))?;
        Ok(manifest)
    }

    fn write_manifest(&self, manifest_path: &Path, manifest: &TranscriptManifest) -> Result<()> {
        let manifest_json = serde_json::to_string_pretty(manifest)
            .context("serialize transcript manifest")?;
        fs::write(manifest_path, manifest_json)
            .with_context(|| format!("write transcript manifest {}", manifest_path.display()))?;
        Ok(())
    }
}
