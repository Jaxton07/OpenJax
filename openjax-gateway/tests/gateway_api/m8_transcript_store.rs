use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use openjax_gateway::transcript::{
    TRANSCRIPT_SCHEMA_VERSION, TranscriptRecord, TranscriptStore,
};
use serde_json::json;

fn temp_transcript_root() -> PathBuf {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock drift")
        .as_nanos();
    std::env::temp_dir().join(format!("openjax-gateway-m8-{pid}-{nanos}"))
}

#[test]
fn transcript_store_creates_manifest_and_first_segment() {
    let root = temp_transcript_root();
    let store = TranscriptStore::new(root.clone());
    let record = TranscriptRecord {
        schema_version: TRANSCRIPT_SCHEMA_VERSION,
        session_id: "sess_m8".to_string(),
        event_seq: 1,
        turn_seq: 1,
        turn_id: Some("turn_1".to_string()),
        event_type: "user_message".to_string(),
        stream_source: "gateway".to_string(),
        timestamp: "2026-03-30T10:00:00Z".to_string(),
        payload: json!({"text":"hello"}),
        request_id: "req_m8".to_string(),
    };

    store.append(&record).expect("append first transcript record");

    let session_root = root.join("sessions").join("sess_m8");
    let manifest_path = session_root.join("manifest.json");
    let first_segment_path = session_root
        .join("segments")
        .join("segment-000001.jsonl");

    assert!(manifest_path.is_file(), "manifest should be created");
    assert!(
        first_segment_path.is_file(),
        "first segment should be created"
    );

    let manifest_raw = fs::read_to_string(&manifest_path).expect("read manifest");
    let manifest_json: serde_json::Value =
        serde_json::from_str(&manifest_raw).expect("parse manifest json");
    assert_eq!(manifest_json["session_id"], "sess_m8");
    assert_eq!(manifest_json["active_segment"], "segment-000001.jsonl");

    let segment_raw = fs::read_to_string(&first_segment_path).expect("read segment");
    let first_line = segment_raw.lines().next().expect("segment first line");
    let record_json: serde_json::Value =
        serde_json::from_str(first_line).expect("parse segment line");
    assert_eq!(record_json["event_seq"], 1);
    assert_eq!(record_json["event_type"], "user_message");

    let _ = fs::remove_dir_all(root);
}
