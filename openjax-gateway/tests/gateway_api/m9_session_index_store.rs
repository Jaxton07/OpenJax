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

fn index_entry(session_id: &str, updated_at: &str, last_event_seq: u64) -> IndexSessionEntry {
    IndexSessionEntry {
        session_id: session_id.to_string(),
        title: Some(format!("title-{session_id}")),
        created_at: "2026-03-30T08:00:00.000Z".to_string(),
        updated_at: updated_at.to_string(),
        last_event_seq,
        last_preview: format!("preview-{session_id}"),
    }
}

fn read_log_records(root: &PathBuf) -> Vec<serde_json::Value> {
    let log_path = root.join("sessions").join("index.log.ndjson");
    if !log_path.exists() {
        return Vec::new();
    }
    let content = fs::read_to_string(log_path).expect("read index log");
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("parse log line"))
        .collect()
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
        sessions
            .iter()
            .map(|entry| entry.session_id.as_str())
            .collect::<Vec<_>>(),
        vec!["sess_c", "sess_b"]
    );
    assert_eq!(sessions[1].last_event_seq, 4);
    assert_eq!(sessions[1].last_preview, "beta-updated");

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn create_session_uses_staging_then_publish() {
    let root = temp_transcript_root();
    let store = SessionIndexStore::new(root.clone()).expect("build index store");
    let entry = index_entry("sess_create", "2026-03-30T12:00:00.000Z", 0);

    store
        .create_session_index_entry(entry.clone())
        .await
        .expect("create session index entry");

    let published = root
        .join("sessions")
        .join("sess_create")
        .join("session.json");
    let staging = root.join("sessions").join(".staging").join("sess_create");
    assert!(published.is_file(), "published session.json should exist");
    assert!(
        !staging.exists(),
        "staging directory should be removed after publish"
    );
    assert_eq!(store.list_sessions(), vec![entry]);

    let records = read_log_records(&root);
    assert_eq!(records.len(), 1, "create should append one upsert record");
    assert_eq!(records[0]["op"], "upsert_session");
    assert_eq!(records[0]["session_id"], "sess_create");

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn delete_session_rolls_back_index_when_remove_dir_fails() {
    let root = temp_transcript_root();
    let store = SessionIndexStore::new(root.clone()).expect("build index store");
    let entry = index_entry("sess_delete_rollback", "2026-03-30T12:00:00.000Z", 2);
    store
        .create_session_index_entry(entry.clone())
        .await
        .expect("seed index entry");

    let session_path = root.join("sessions").join("sess_delete_rollback");
    fs::remove_dir_all(&session_path).expect("remove seeded session dir");
    fs::write(&session_path, b"force-not-a-dir").expect("replace session path with file");

    let err = store
        .delete_session_index_entry("sess_delete_rollback")
        .await
        .expect_err("delete should fail when remove_dir_all fails");
    let err_text = format!("{err:#}");
    assert!(
        err_text.contains("remove session directory"),
        "error should come from remove_dir_all: {err_text}"
    );

    assert_eq!(
        store
            .list_sessions()
            .iter()
            .map(|entry| entry.session_id.as_str())
            .collect::<Vec<_>>(),
        vec!["sess_delete_rollback"],
        "index entry should be restored by compensation upsert"
    );

    let records = read_log_records(&root);
    assert_eq!(records.len(), 3, "create + delete + compensation upsert");
    assert_eq!(records[1]["op"], "delete_session");
    assert_eq!(records[2]["op"], "upsert_session");
    assert_eq!(records[2]["session_id"], "sess_delete_rollback");

    let _ = fs::remove_file(&session_path);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn compensation_append_failure_enters_index_repair_required() {
    let root = temp_transcript_root();
    let store = SessionIndexStore::new_with_test_fail_append_nth(root.clone(), Some(2))
        .expect("build index store");
    let entry = index_entry("sess_repair_required", "2026-03-30T12:10:00.000Z", 0);

    let published_path = root.join("sessions").join("sess_repair_required");
    fs::create_dir_all(published_path.parent().expect("session parent")).expect("create sessions");
    fs::write(&published_path, b"conflict-file").expect("seed publish conflict file");

    store
        .create_session_index_entry(entry)
        .await
        .expect_err("create should fail when publish + compensation fail");
    assert!(
        store.is_repair_required(),
        "store should enter repair required state"
    );

    let next_err = store
        .delete_session_index_entry("sess_repair_required")
        .await
        .expect_err("writes should be rejected in repair required state");
    assert!(
        format!("{next_err:#}").contains("index_repair_required"),
        "write rejection should expose repair-required marker"
    );

    let _ = fs::remove_file(&published_path);
    let _ = fs::remove_dir_all(root);
}
