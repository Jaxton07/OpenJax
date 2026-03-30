use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

fn write_session_json(root: &PathBuf, session_id: &str, updated_at: &str) {
    let session_root = root.join("sessions").join(session_id);
    fs::create_dir_all(&session_root).expect("create session root");
    let payload = json!({
        "schema_version": 1,
        "session_id": session_id,
        "title": format!("title-{session_id}"),
        "created_at": "2026-03-30T08:00:00.000Z",
        "updated_at": updated_at,
        "tags": []
    });
    fs::write(
        session_root.join("session.json"),
        serde_json::to_string_pretty(&payload).expect("serialize session.json"),
    )
    .expect("write session.json");
}

fn write_manifest(root: &PathBuf, session_id: &str, last_event_seq: u64, updated_at: &str) {
    let session_root = root.join("sessions").join(session_id);
    fs::create_dir_all(&session_root).expect("create session root for manifest");
    let payload = json!({
        "schema_version": 1,
        "session_id": session_id,
        "last_event_seq": last_event_seq,
        "last_turn_seq": 0,
        "active_segment": "segment-000001.jsonl",
        "updated_at": updated_at
    });
    fs::write(
        session_root.join("manifest.json"),
        serde_json::to_string_pretty(&payload).expect("serialize manifest"),
    )
    .expect("write manifest");
}

fn set_modified_at(path: &PathBuf, modified: SystemTime) {
    let file = fs::File::open(path).expect("open path for set_times");
    file.set_times(fs::FileTimes::new().set_modified(modified))
        .expect("set modified time");
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
    write_session_json(&root, "sess_b", "2026-03-30T11:00:00.000Z");
    write_manifest(&root, "sess_b", 4, "2026-03-30T11:00:00.000Z");
    write_session_json(&root, "sess_c", "2026-03-30T11:00:00.000Z");
    write_manifest(&root, "sess_c", 2, "2026-03-30T11:00:00.000Z");

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

#[tokio::test]
async fn compact_rotates_log_with_tmp_and_bak_without_truncating_live_log() {
    let root = temp_transcript_root();
    let sessions_root = root.join("sessions");
    fs::create_dir_all(&sessions_root).expect("create sessions root");

    let mut log_content = String::new();
    for idx in 0..1000 {
        let record = json!({
            "op": "upsert_session",
            "session_id": format!("seed_{idx}"),
            "ts": "2026-03-30T11:00:00.000Z",
            "payload": entry(
                &format!("seed_{idx}"),
                "2026-03-30T11:00:00.000Z",
                idx as u64,
                "seed"
            )
        });
        log_content.push_str(
            &serde_json::to_string(&record).expect("serialize seeded compact log record"),
        );
        log_content.push('\n');
    }
    fs::write(sessions_root.join("index.log.ndjson"), log_content).expect("write seeded log");

    let store = SessionIndexStore::new(root.clone()).expect("build index store");
    store
        .create_session_index_entry(index_entry("sess_compact", "2026-03-30T13:00:00.000Z", 1))
        .await
        .expect("create entry to trigger compact");

    let compacted_log = fs::read_to_string(sessions_root.join("index.log.ndjson"))
        .expect("read compacted log");
    assert!(
        compacted_log.lines().count() < 1000,
        "log should be compacted instead of accumulating >1000 lines"
    );
    assert!(
        !sessions_root.join("index.log.ndjson.tmp").exists(),
        "compact should clean tmp log file"
    );
    assert!(
        !sessions_root.join("index.log.ndjson.bak").exists(),
        "compact should clean bak log file"
    );
    let snapshot_raw = fs::read_to_string(sessions_root.join("index.snapshot.json"))
        .expect("compact should persist snapshot");
    let snapshot: serde_json::Value =
        serde_json::from_str(&snapshot_raw).expect("parse compacted snapshot");
    assert!(
        snapshot["sessions"]
            .as_array()
            .expect("snapshot sessions array")
            .iter()
            .any(|item| item["session_id"] == "sess_compact"),
        "compacted snapshot should contain latest session"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn startup_audit_reconciles_index_and_session_dirs() {
    let root = temp_transcript_root();
    let sessions_root = root.join("sessions");
    fs::create_dir_all(&sessions_root).expect("create sessions root");

    let snapshot = json!({
        "schema_version": 1,
        "updated_at": "2026-03-30T10:00:00.000Z",
        "sessions": [entry("sess_ghost", "2026-03-30T10:00:00.000Z", 1, "ghost")]
    });
    fs::write(
        sessions_root.join("index.snapshot.json"),
        serde_json::to_string_pretty(&snapshot).expect("serialize snapshot"),
    )
    .expect("write snapshot");
    write_session_json(&root, "sess_present", "2026-03-30T11:00:00.000Z");
    write_manifest(&root, "sess_present", 5, "2026-03-30T11:00:00.000Z");

    let store = SessionIndexStore::new(root.clone()).expect("startup should reconcile");
    assert_eq!(
        store
            .list_sessions()
            .into_iter()
            .map(|item| item.session_id)
            .collect::<Vec<_>>(),
        vec!["sess_present".to_string()],
        "startup audit should drop missing dir entry and add missing index entry"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebuild_from_sessions_dir_recovers_when_snapshot_corrupted() {
    let root = temp_transcript_root();
    let sessions_root = root.join("sessions");
    fs::create_dir_all(&sessions_root).expect("create sessions root");
    fs::write(
        sessions_root.join("index.snapshot.json"),
        "{invalid-json",
    )
    .expect("write corrupted snapshot");
    write_session_json(&root, "sess_rebuild_snapshot", "2026-03-30T11:30:00.000Z");
    write_manifest(&root, "sess_rebuild_snapshot", 9, "2026-03-30T11:30:00.000Z");

    let store = SessionIndexStore::new(root.clone()).expect("snapshot corruption should rebuild");
    let sessions = store.list_sessions();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, "sess_rebuild_snapshot");
    assert_eq!(sessions[0].last_event_seq, 9);

    let repaired_snapshot = fs::read_to_string(sessions_root.join("index.snapshot.json"))
        .expect("snapshot should be rewritten during rebuild");
    serde_json::from_str::<serde_json::Value>(&repaired_snapshot).expect("snapshot should be json");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebuild_from_sessions_dir_runs_when_log_replay_corrupted() {
    let root = temp_transcript_root();
    let sessions_root = root.join("sessions");
    fs::create_dir_all(&sessions_root).expect("create sessions root");

    let snapshot = json!({
        "schema_version": 1,
        "updated_at": "2026-03-30T10:00:00.000Z",
        "sessions": []
    });
    fs::write(
        sessions_root.join("index.snapshot.json"),
        serde_json::to_string_pretty(&snapshot).expect("serialize snapshot"),
    )
    .expect("write snapshot");
    fs::write(
        sessions_root.join("index.log.ndjson"),
        "{\"op\":\"upsert_session\"}\nnot-json\n",
    )
    .expect("write corrupted log");
    write_session_json(&root, "sess_rebuild_log", "2026-03-30T12:00:00.000Z");
    write_manifest(&root, "sess_rebuild_log", 3, "2026-03-30T12:00:00.000Z");

    let store = SessionIndexStore::new(root.clone()).expect("log corruption should rebuild");
    assert_eq!(
        store
            .list_sessions()
            .iter()
            .map(|item| item.session_id.as_str())
            .collect::<Vec<_>>(),
        vec!["sess_rebuild_log"]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn startup_fails_when_rebuild_fails() {
    let root = temp_transcript_root();
    let sessions_root = root.join("sessions");
    fs::create_dir_all(&sessions_root).expect("create sessions root");
    fs::write(
        sessions_root.join("index.snapshot.json"),
        "{invalid-json",
    )
    .expect("write corrupted snapshot");
    let bad_session_root = sessions_root.join("sess_bad_rebuild");
    fs::create_dir_all(&bad_session_root).expect("create bad session dir");
    fs::write(bad_session_root.join("session.json"), "{bad").expect("write bad session metadata");

    let err = SessionIndexStore::new(root.clone()).expect_err("startup should fail when rebuild fails");
    assert!(
        format!("{err:#}").contains("rebuild_from_sessions_dir"),
        "error should indicate rebuild failure path: {err:#}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn startup_cleanup_removes_stale_staging_dirs_and_keeps_recent_ones() {
    let root = temp_transcript_root();
    let staging_root = root.join("sessions").join(".staging");
    let stale = staging_root.join("sess_stale");
    let recent = staging_root.join("sess_recent");
    fs::create_dir_all(&stale).expect("create stale staging");
    fs::create_dir_all(&recent).expect("create recent staging");
    fs::write(stale.join("session.json"), "{}").expect("seed stale staging");
    fs::write(recent.join("session.json"), "{}").expect("seed recent staging");

    let old = SystemTime::now() - Duration::from_secs(11 * 60);
    set_modified_at(&stale, old);

    let _ = SessionIndexStore::new(root.clone()).expect("startup should clean staging");
    assert!(
        !stale.exists(),
        "stale staging directory (>10 minutes) should be removed"
    );
    assert!(
        recent.exists(),
        "recent staging directory (<=10 minutes) should be kept"
    );

    let _ = fs::remove_dir_all(root);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_upsert_and_touch_do_not_corrupt_index_log() {
    let root = temp_transcript_root();
    let store = Arc::new(SessionIndexStore::new(root.clone()).expect("build index store"));
    store
        .create_session_index_entry(index_entry("sess_concurrent", "2026-03-30T12:00:00.000Z", 0))
        .await
        .expect("seed concurrent session");

    let mut tasks = Vec::new();
    for task_id in 0..8usize {
        let store = Arc::clone(&store);
        tasks.push(tokio::spawn(async move {
            for step in 0..20usize {
                let seq = (task_id * 100 + step) as u64;
                let updated_at = format!("2026-03-30T12:{:02}:{:02}.000Z", task_id, step % 60);
                let upsert_entry = IndexSessionEntry {
                    session_id: "sess_concurrent".to_string(),
                    title: Some("title-sess_concurrent".to_string()),
                    created_at: "2026-03-30T08:00:00.000Z".to_string(),
                    updated_at: updated_at.clone(),
                    last_event_seq: seq,
                    last_preview: format!("preview-upsert-{task_id}-{step}"),
                };
                store
                    .upsert_session_index_entry(upsert_entry)
                    .await
                    .expect("upsert should succeed");
                store
                    .touch_session_index_entry(
                        "sess_concurrent",
                        format!("2026-03-30T13:{:02}:{:02}.000Z", task_id, step % 60),
                        seq + 1,
                        format!("preview-touch-{task_id}-{step}"),
                    )
                    .await
                    .expect("touch should succeed");
            }
        }));
    }
    for task in tasks {
        task.await.expect("join task");
    }

    let records = read_log_records(&root);
    assert!(
        records.len() >= 1 + (8 * 20 * 2),
        "every upsert/touch should append a durable log record"
    );
    for (idx, record) in records.iter().enumerate() {
        assert!(
            record.get("op").is_some(),
            "log line {idx} should remain parseable under concurrency"
        );
    }
    let sessions = store.list_sessions();
    assert_eq!(sessions.len(), 1, "concurrent upsert/touch should keep single entry");
    assert_eq!(sessions[0].session_id, "sess_concurrent");

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn compact_rollback_failure_sets_repair_required() {
    let root = temp_transcript_root();
    let sessions_root = root.join("sessions");
    fs::create_dir_all(&sessions_root).expect("create sessions root");

    let mut log_content = String::new();
    for idx in 0..1000 {
        let record = json!({
            "op": "upsert_session",
            "session_id": format!("seed_fail_{idx}"),
            "ts": "2026-03-30T11:00:00.000Z",
            "payload": entry(
                &format!("seed_fail_{idx}"),
                "2026-03-30T11:00:00.000Z",
                idx as u64,
                "seed"
            )
        });
        log_content.push_str(
            &serde_json::to_string(&record).expect("serialize seeded compact rollback log"),
        );
        log_content.push('\n');
    }
    fs::write(sessions_root.join("index.log.ndjson"), log_content).expect("write seeded log");

    let store = SessionIndexStore::new_with_test_failures(root.clone(), None, true, true)
        .expect("build index store with compact rollback failure injection");
    let err = store
        .create_session_index_entry(index_entry(
            "sess_compact_failure",
            "2026-03-30T13:20:00.000Z",
            2,
        ))
        .await
        .expect_err("compact rollback failure should fail create");
    assert!(
        format!("{err:#}").contains("index_repair_required"),
        "compact rollback failure should expose repair marker: {err:#}"
    );
    assert!(
        store.is_repair_required(),
        "rollback failure should move store into repair-required mode"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn startup_audit_persists_repair_ops_into_index_log() {
    let root = temp_transcript_root();
    let sessions_root = root.join("sessions");
    fs::create_dir_all(&sessions_root).expect("create sessions root");

    let snapshot = json!({
        "schema_version": 1,
        "updated_at": "2026-03-30T10:00:00.000Z",
        "sessions": [entry("sess_deleted_by_audit", "2026-03-30T09:30:00.000Z", 1, "ghost")]
    });
    fs::write(
        sessions_root.join("index.snapshot.json"),
        serde_json::to_string_pretty(&snapshot).expect("serialize snapshot"),
    )
    .expect("write snapshot");
    write_session_json(&root, "sess_added_by_audit", "2026-03-30T11:05:00.000Z");
    write_manifest(&root, "sess_added_by_audit", 6, "2026-03-30T11:05:00.000Z");

    let _ = SessionIndexStore::new(root.clone()).expect("startup should reconcile and persist");
    let records = read_log_records(&root);
    assert!(
        records.iter().any(|record| {
            record["op"] == "delete_session" && record["session_id"] == "sess_deleted_by_audit"
        }),
        "audit should persist delete repair operation"
    );
    assert!(
        records.iter().any(|record| {
            record["op"] == "upsert_session" && record["session_id"] == "sess_added_by_audit"
        }),
        "audit should persist upsert repair operation"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rebuild_ignores_sessions_under_staging_directory() {
    let root = temp_transcript_root();
    let sessions_root = root.join("sessions");
    fs::create_dir_all(&sessions_root).expect("create sessions root");
    fs::write(sessions_root.join("index.snapshot.json"), "{invalid-json")
        .expect("write corrupted snapshot to force rebuild");

    write_session_json(&root, "sess_real", "2026-03-30T14:00:00.000Z");
    write_manifest(&root, "sess_real", 4, "2026-03-30T14:00:00.000Z");
    let staging_session = sessions_root.join(".staging").join("sess_shadow");
    fs::create_dir_all(&staging_session).expect("create staging session");
    fs::write(
        staging_session.join("session.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "session_id": "sess_shadow",
            "title": "shadow",
            "created_at": "2026-03-30T08:00:00.000Z",
            "updated_at": "2026-03-30T14:01:00.000Z",
            "tags": []
        }))
        .expect("serialize staging session"),
    )
    .expect("write staging session metadata");

    let store = SessionIndexStore::new(root.clone()).expect("rebuild should succeed");
    assert_eq!(
        store
            .list_sessions()
            .into_iter()
            .map(|item| item.session_id)
            .collect::<Vec<_>>(),
        vec!["sess_real".to_string()],
        "rebuild should ignore sessions under .staging"
    );

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn restart_rebuild_clears_previous_repair_required_state() {
    let root = temp_transcript_root();
    let broken_store = SessionIndexStore::new_with_test_fail_append_nth(root.clone(), Some(2))
        .expect("build store");
    let published_path = root.join("sessions").join("sess_restart_repair");
    fs::create_dir_all(published_path.parent().expect("session parent")).expect("create sessions");
    fs::write(&published_path, b"conflict-file").expect("seed publish conflict file");

    broken_store
        .create_session_index_entry(index_entry(
            "sess_restart_repair",
            "2026-03-30T12:10:00.000Z",
            0,
        ))
        .await
        .expect_err("force repair-required on first process");
    assert!(broken_store.is_repair_required());

    let _ = fs::remove_file(&published_path);
    fs::write(
        root.join("sessions").join("index.snapshot.json"),
        "{invalid-json",
    )
    .expect("corrupt snapshot to force rebuild on restart");
    write_session_json(&root, "sess_restart_ok", "2026-03-30T15:00:00.000Z");
    write_manifest(&root, "sess_restart_ok", 8, "2026-03-30T15:00:00.000Z");

    let restarted = SessionIndexStore::new(root.clone()).expect("restart should rebuild");
    assert!(
        !restarted.is_repair_required(),
        "successful restart rebuild should clear repair-required state"
    );
    assert_eq!(
        restarted
            .list_sessions()
            .into_iter()
            .map(|item| item.session_id)
            .collect::<Vec<_>>(),
        vec!["sess_restart_ok".to_string()]
    );

    let _ = fs::remove_dir_all(root);
}
