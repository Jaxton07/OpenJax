use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use openjax_gateway::transcript::{IndexSessionEntry, SessionIndexStore};
use serde_json::json;

fn temp_transcript_root() -> PathBuf {
    static UNIQUIFIER: AtomicU64 = AtomicU64::new(0);
    let pid = std::process::id();
    let uniq = UNIQUIFIER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock drift")
        .as_nanos();
    std::env::temp_dir().join(format!("openjax-gateway-m9-{pid}-{nanos}-{uniq}"))
}

fn entry(
    session_id: &str,
    updated_at: &str,
    last_event_seq: u64,
    last_preview: &str,
) -> serde_json::Value {
    json!({
        "session_id": session_id,
        "title": format!("title-{session_id}"),
        "created_at": "2026-03-30T08:00:00.000Z",
        "updated_at": updated_at,
        "last_event_seq": last_event_seq,
        "last_preview": last_preview
    })
}

#[test]
fn index_store_recovers_from_snapshot_and_log() {
    let root = temp_transcript_root();
    let sessions_root = root.join("sessions");
    fs::create_dir_all(&sessions_root).expect("create sessions root");

    let snapshot_path = sessions_root.join("index.snapshot.json");
    let log_path = sessions_root.join("index.log.ndjson");

    let snapshot = json!({
        "schema_version": 1,
        "updated_at": "2026-03-30T10:00:00.000Z",
        "sessions": [
            entry("sess_a", "2026-03-30T10:00:00.000Z", 3, "alpha"),
            entry("sess_b", "2026-03-30T09:00:00.000Z", 1, "beta")
        ]
    });
    fs::write(
        &snapshot_path,
        serde_json::to_string_pretty(&snapshot).expect("serialize snapshot"),
    )
    .expect("write snapshot");

    let upsert_sess_b = json!({
        "op": "upsert_session",
        "session_id": "sess_b",
        "ts": "2026-03-30T11:00:00.000Z",
        "payload": entry("sess_b", "2026-03-30T11:00:00.000Z", 4, "beta-updated")
    });
    let delete_sess_a = json!({
        "op": "delete_session",
        "session_id": "sess_a",
        "ts": "2026-03-30T11:00:01.000Z"
    });
    let upsert_sess_c = json!({
        "op": "upsert_session",
        "session_id": "sess_c",
        "ts": "2026-03-30T11:00:02.000Z",
        "payload": entry("sess_c", "2026-03-30T11:00:00.000Z", 2, "charlie")
    });
    let log_content = format!(
        "{}\n{}\n{}\n",
        serde_json::to_string(&upsert_sess_b).expect("serialize upsert sess_b"),
        serde_json::to_string(&delete_sess_a).expect("serialize delete sess_a"),
        serde_json::to_string(&upsert_sess_c).expect("serialize upsert sess_c")
    );
    fs::write(&log_path, log_content).expect("write log");

    let store = SessionIndexStore::new(root.clone()).expect("build index store from disk");
    let sessions: Vec<IndexSessionEntry> = store.list_sessions();

    assert_eq!(
        sessions.iter().map(|entry| entry.session_id.as_str()).collect::<Vec<_>>(),
        vec!["sess_c", "sess_b"]
    );
    assert_eq!(sessions[1].last_event_seq, 4);
    assert_eq!(sessions[1].last_preview, "beta-updated");

    let _ = fs::remove_dir_all(root);
}
