use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use openjax_gateway::transcript::{
    FIRST_SEGMENT_FILE, TRANSCRIPT_SCHEMA_VERSION, TranscriptRecord, TranscriptStore,
};
use serde_json::json;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

fn temp_transcript_root() -> PathBuf {
    static UNIQUIFIER: AtomicU64 = AtomicU64::new(0);
    let pid = std::process::id();
    let uniq = UNIQUIFIER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock drift")
        .as_nanos();
    std::env::temp_dir().join(format!("openjax-gateway-m8-{pid}-{nanos}-{uniq}"))
}

fn rfc3339_days_ago(days: i64) -> String {
    (OffsetDateTime::now_utc() - Duration::days(days))
        .format(&Rfc3339)
        .expect("format rfc3339")
}

fn new_record(
    session_id: &str,
    turn_seq: u64,
    request_id: &str,
    timestamp: String,
) -> TranscriptRecord {
    TranscriptRecord {
        schema_version: TRANSCRIPT_SCHEMA_VERSION,
        session_id: session_id.to_string(),
        event_seq: 999,
        turn_seq,
        turn_id: Some(format!("turn_{turn_seq}")),
        event_type: "user_message".to_string(),
        stream_source: "gateway".to_string(),
        timestamp,
        payload: json!({"text":"hello"}),
        request_id: request_id.to_string(),
    }
}

fn load_manifest(root: &PathBuf, session_id: &str) -> serde_json::Value {
    let manifest_path = root.join("sessions").join(session_id).join("manifest.json");
    let manifest_raw = fs::read_to_string(&manifest_path).expect("read manifest");
    serde_json::from_str(&manifest_raw).expect("parse manifest")
}

#[test]
fn transcript_store_creates_manifest_and_first_segment() {
    let root = temp_transcript_root();
    let store = TranscriptStore::new(root.clone());
    store
        .append(&new_record("sess_m8", 1, "req_m8", rfc3339_days_ago(0)))
        .expect("append first transcript record");

    let session_root = root.join("sessions").join("sess_m8");
    let manifest_path = session_root.join("manifest.json");
    let first_segment_path = session_root.join("segments").join(FIRST_SEGMENT_FILE);

    assert!(manifest_path.is_file(), "manifest should be created");
    assert!(
        first_segment_path.is_file(),
        "first segment should be created"
    );

    let manifest_raw = fs::read_to_string(&manifest_path).expect("read manifest");
    let manifest_json: serde_json::Value =
        serde_json::from_str(&manifest_raw).expect("parse manifest json");
    assert_eq!(manifest_json["session_id"], "sess_m8");
    assert_eq!(manifest_json["active_segment"], FIRST_SEGMENT_FILE);

    let segment_raw = fs::read_to_string(&first_segment_path).expect("read segment");
    let first_line = segment_raw.lines().next().expect("segment first line");
    let record_json: serde_json::Value =
        serde_json::from_str(first_line).expect("parse segment line");
    assert_eq!(record_json["event_seq"], 1);
    assert_eq!(record_json["event_type"], "user_message");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn append_assigns_monotonic_event_seq_per_session() {
    let root = temp_transcript_root();
    let store = TranscriptStore::new(root.clone());

    let first = store
        .append(&new_record("sess_seq", 1, "req_1", rfc3339_days_ago(0)))
        .expect("append first");
    let second = store
        .append(&new_record("sess_seq", 1, "req_2", rfc3339_days_ago(0)))
        .expect("append second");
    let third = store
        .append(&new_record("sess_seq", 2, "req_3", rfc3339_days_ago(0)))
        .expect("append third");

    assert_eq!(first.event_seq, 1);
    assert_eq!(second.event_seq, 2);
    assert_eq!(third.event_seq, 3);

    let replay = store.replay("sess_seq", None).expect("replay");
    let seqs: Vec<u64> = replay.iter().map(|r| r.event_seq).collect();
    assert_eq!(seqs, vec![1, 2, 3]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rotates_segment_when_size_limit_reached() {
    let root = temp_transcript_root();
    let store = TranscriptStore::with_segment_max_bytes(root.clone(), 450);

    let mut payload = String::new();
    for _ in 0..180 {
        payload.push('x');
    }

    let mut first = new_record("sess_rotate", 1, "req_rotate_1", rfc3339_days_ago(0));
    first.payload = json!({ "blob": payload.clone() });
    let mut second = new_record("sess_rotate", 1, "req_rotate_2", rfc3339_days_ago(0));
    second.payload = json!({ "blob": payload });

    store.append(&first).expect("append first");
    store.append(&second).expect("append second");

    let manifest = load_manifest(&root, "sess_rotate");
    assert_eq!(manifest["active_segment"], "segment-000002.jsonl");

    let first_segment = root
        .join("sessions")
        .join("sess_rotate")
        .join("segments")
        .join("segment-000001.jsonl");
    let second_segment = root
        .join("sessions")
        .join("sess_rotate")
        .join("segments")
        .join("segment-000002.jsonl");
    assert!(first_segment.is_file(), "first segment should exist");
    assert!(second_segment.is_file(), "rotated segment should exist");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn gc_deletes_records_older_than_30_days() {
    let root = temp_transcript_root();
    let store = TranscriptStore::new(root.clone());

    store
        .append(&new_record("sess_old", 1, "req_old", rfc3339_days_ago(45)))
        .expect("append old");
    store
        .append(&new_record("sess_new", 1, "req_new", rfc3339_days_ago(1)))
        .expect("append new");

    store.gc(30).expect("gc");

    assert!(
        !root.join("sessions").join("sess_old").exists(),
        "old session should be removed"
    );
    assert!(
        root.join("sessions").join("sess_new").exists(),
        "new session should be retained"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn recovers_manifest_seq_when_tail_record_is_newer() {
    let root = temp_transcript_root();
    let store = TranscriptStore::new(root.clone());
    store
        .append(&new_record("sess_recover", 1, "req_1", rfc3339_days_ago(0)))
        .expect("append first");
    store
        .append(&new_record("sess_recover", 1, "req_2", rfc3339_days_ago(0)))
        .expect("append second");

    let manifest_path = root
        .join("sessions")
        .join("sess_recover")
        .join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    manifest["last_event_seq"] = json!(1u64);
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
    )
    .expect("write stale manifest");

    let warnings = store
        .recover_manifest_from_active_segment_tail("sess_recover")
        .expect("recover manifest");
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("manifest_tail_recovered")),
        "expected manifest recovery warning"
    );

    let repaired = load_manifest(&root, "sess_recover");
    assert_eq!(repaired["last_event_seq"], 2);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn switches_segment_and_warns_when_active_segment_is_not_writable() {
    let root = temp_transcript_root();
    let store = TranscriptStore::new(root.clone());

    store
        .append(&new_record(
            "sess_unwritable",
            1,
            "req_unwritable_1",
            rfc3339_days_ago(0),
        ))
        .expect("append first");

    let session_root = root.join("sessions").join("sess_unwritable");
    let segments_root = session_root.join("segments");
    let active_segment = segments_root.join("segment-000001.jsonl");
    fs::remove_file(&active_segment).expect("remove active segment file");
    fs::create_dir_all(&active_segment).expect("replace segment with directory");

    let warnings = store
        .rotate_when_active_segment_unwritable("sess_unwritable")
        .expect("rotate when unwritable");
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("active_segment_unwritable_rotated")),
        "expected unwritable-segment warning"
    );

    let manifest = load_manifest(&root, "sess_unwritable");
    assert_eq!(manifest["active_segment"], "segment-000002.jsonl");

    let _ = fs::remove_dir_all(root);
}
