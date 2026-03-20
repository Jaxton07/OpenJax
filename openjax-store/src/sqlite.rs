use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::repository::{ProviderRepository, SessionRepository};
use crate::types::{
    ActiveProviderRecord, EventRecord, MessageRecord, ProviderRecord, SessionRecord,
};

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

/// Shared SQLite store for sessions, messages, events, and LLM provider config.
/// This is the single source of truth for all persisted state in OpenJax.
#[derive(Clone)]
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create store db dir {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("open store db {}", path.display()))?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema()?;
        {
            let conn = store.conn.lock().expect("store db mutex poisoned");
            Self::migrate_schema(&conn).context("migrate store schema")?;
        }
        Ok(store)
    }

    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("open in-memory store db")?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema()?;
        {
            let conn = store.conn.lock().expect("store db mutex poisoned");
            Self::migrate_schema(&conn).context("migrate store schema")?;
        }
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().expect("store db mutex poisoned");
        conn.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS biz_sessions (
                session_id TEXT PRIMARY KEY,
                title TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_biz_sessions_updated_at ON biz_sessions(updated_at DESC);

            CREATE TABLE IF NOT EXISTS biz_messages (
                message_id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                turn_id TEXT,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                sequence INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY(session_id) REFERENCES biz_sessions(session_id) ON DELETE CASCADE
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_biz_messages_session_seq
                ON biz_messages(session_id, sequence);
            CREATE INDEX IF NOT EXISTS idx_biz_messages_session_created_at
                ON biz_messages(session_id, created_at ASC);

            CREATE TABLE IF NOT EXISTS biz_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                event_seq INTEGER NOT NULL,
                turn_seq INTEGER NOT NULL DEFAULT 0,
                turn_id TEXT,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                stream_source TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY(session_id) REFERENCES biz_sessions(session_id) ON DELETE CASCADE
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_biz_events_session_event_seq
                ON biz_events(session_id, event_seq);
            CREATE INDEX IF NOT EXISTS idx_biz_events_session_turn_event_seq
                ON biz_events(session_id, turn_id, event_seq);
            CREATE INDEX IF NOT EXISTS idx_biz_events_session_created_at
                ON biz_events(session_id, created_at ASC);

            CREATE TABLE IF NOT EXISTS llm_providers (
                provider_id TEXT PRIMARY KEY,
                provider_name TEXT NOT NULL UNIQUE,
                base_url TEXT NOT NULL,
                model_name TEXT NOT NULL,
                api_key TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_llm_providers_name
                ON llm_providers(provider_name);

            CREATE TABLE IF NOT EXISTS llm_runtime_settings (
                setting_key TEXT PRIMARY KEY,
                provider_id TEXT,
                model_name TEXT,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(provider_id) REFERENCES llm_providers(provider_id) ON DELETE SET NULL
            );
            "#,
        )
        .context("init store schema")?;
        Ok(())
    }

    pub(crate) fn migrate_schema(conn: &rusqlite::Connection) -> anyhow::Result<()> {
        // llm_providers: add provider_type
        let has_provider_type: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('llm_providers') WHERE name = 'provider_type'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;
        if !has_provider_type {
            conn.execute_batch(
                "ALTER TABLE llm_providers ADD COLUMN provider_type TEXT NOT NULL DEFAULT 'custom'",
            )
            .context("migrate: add provider_type to llm_providers")?;
        }

        // llm_providers: add context_window_size
        let has_provider_cw: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('llm_providers') WHERE name = 'context_window_size'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;
        if !has_provider_cw {
            conn.execute_batch(
                "ALTER TABLE llm_providers ADD COLUMN context_window_size INTEGER NOT NULL DEFAULT 0",
            )
            .context("migrate: add context_window_size to llm_providers")?;
        }

        // llm_runtime_settings: add context_window_size
        let has_settings_cw: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('llm_runtime_settings') WHERE name = 'context_window_size'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;
        if !has_settings_cw {
            conn.execute_batch(
                "ALTER TABLE llm_runtime_settings ADD COLUMN context_window_size INTEGER NOT NULL DEFAULT 0",
            )
            .context("migrate: add context_window_size to llm_runtime_settings")?;
        }

        Ok(())
    }

    fn next_message_sequence(tx: &rusqlite::Transaction<'_>, session_id: &str) -> Result<i64> {
        let current_max: Option<i64> = tx
            .query_row(
                "SELECT MAX(sequence) FROM biz_messages WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .optional()
            .context("query current max message sequence")?
            .flatten();
        Ok(current_max.unwrap_or(0) + 1)
    }
}

impl SessionRepository for SqliteStore {
    fn create_session(&self, session_id: &str, title: Option<&str>) -> Result<SessionRecord> {
        {
            let mut conn = self.conn.lock().expect("store db mutex poisoned");
            let tx = conn.transaction().context("begin create session tx")?;
            let now = now_rfc3339();
            tx.execute(
                "INSERT INTO biz_sessions (session_id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
                params![session_id, title, now],
            )
            .with_context(|| format!("insert session {}", session_id))?;
            tx.commit().context("commit create session tx")?;
        }
        self.get_session(session_id)?
            .context("session must exist after create")
    }

    fn get_session(&self, session_id: &str) -> Result<Option<SessionRecord>> {
        let conn = self.conn.lock().expect("store db mutex poisoned");
        conn.query_row(
            "SELECT session_id, title, created_at, updated_at FROM biz_sessions WHERE session_id = ?1",
            params![session_id],
            |row| {
                Ok(SessionRecord {
                    session_id: row.get(0)?,
                    title: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            },
        )
        .optional()
        .context("query session by id")
    }

    fn list_sessions(&self) -> Result<Vec<SessionRecord>> {
        let conn = self.conn.lock().expect("store db mutex poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT session_id, title, created_at, updated_at FROM biz_sessions ORDER BY updated_at DESC",
            )
            .context("prepare list sessions")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(SessionRecord {
                    session_id: row.get(0)?,
                    title: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })
            .context("query list sessions")?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row.context("read session row")?);
        }
        Ok(sessions)
    }

    fn delete_session(&self, session_id: &str) -> Result<bool> {
        let conn = self.conn.lock().expect("store db mutex poisoned");
        let affected = conn
            .execute(
                "DELETE FROM biz_sessions WHERE session_id = ?1",
                params![session_id],
            )
            .context("delete session")?;
        Ok(affected > 0)
    }

    fn append_message(
        &self,
        session_id: &str,
        turn_id: Option<&str>,
        role: &str,
        content: &str,
    ) -> Result<MessageRecord> {
        let message_id = format!("msg_{}", Uuid::new_v4().simple());
        let mut conn = self.conn.lock().expect("store db mutex poisoned");
        let tx = conn.transaction().context("begin append message tx")?;
        let sequence = Self::next_message_sequence(&tx, session_id)?;
        let now = now_rfc3339();
        tx.execute(
            "INSERT INTO biz_messages (message_id, session_id, turn_id, role, content, sequence, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![message_id, session_id, turn_id, role, content, sequence, now],
        )
        .with_context(|| format!("insert message for session {}", session_id))?;
        tx.execute(
            "UPDATE biz_sessions SET updated_at = ?1 WHERE session_id = ?2",
            params![now, session_id],
        )
        .with_context(|| format!("touch session {}", session_id))?;
        tx.commit().context("commit append message tx")?;

        Ok(MessageRecord {
            message_id,
            session_id: session_id.to_string(),
            turn_id: turn_id.map(ToOwned::to_owned),
            role: role.to_string(),
            content: content.to_string(),
            sequence,
            created_at: now,
        })
    }

    fn list_messages(&self, session_id: &str) -> Result<Vec<MessageRecord>> {
        let conn = self.conn.lock().expect("store db mutex poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT message_id, session_id, turn_id, role, content, sequence, created_at
                 FROM biz_messages
                 WHERE session_id = ?1
                 ORDER BY sequence ASC",
            )
            .with_context(|| format!("prepare list messages for session {}", session_id))?;
        let rows = stmt
            .query_map(params![session_id], |row| {
                Ok(MessageRecord {
                    message_id: row.get(0)?,
                    session_id: row.get(1)?,
                    turn_id: row.get(2)?,
                    role: row.get(3)?,
                    content: row.get(4)?,
                    sequence: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .with_context(|| format!("query list messages for session {}", session_id))?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row.context("read message row")?);
        }
        Ok(messages)
    }

    fn append_event(
        &self,
        session_id: &str,
        event_seq: u64,
        turn_seq: u64,
        turn_id: Option<&str>,
        event_type: &str,
        payload_json: &str,
        timestamp: &str,
        stream_source: &str,
    ) -> Result<EventRecord> {
        let mut conn = self.conn.lock().expect("store db mutex poisoned");
        let tx = conn.transaction().context("begin append event tx")?;
        let now = now_rfc3339();
        tx.execute(
            "INSERT INTO biz_events (session_id, event_seq, turn_seq, turn_id, event_type, payload_json, timestamp, stream_source, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                session_id,
                event_seq as i64,
                turn_seq as i64,
                turn_id,
                event_type,
                payload_json,
                timestamp,
                stream_source,
                now
            ],
        )
        .with_context(|| format!("insert event for session {}", session_id))?;
        tx.execute(
            "UPDATE biz_sessions SET updated_at = ?1 WHERE session_id = ?2",
            params![now, session_id],
        )
        .with_context(|| format!("touch session {}", session_id))?;
        let id = tx.last_insert_rowid();
        tx.commit().context("commit append event tx")?;

        Ok(EventRecord {
            id,
            session_id: session_id.to_string(),
            event_seq,
            turn_seq,
            turn_id: turn_id.map(ToOwned::to_owned),
            event_type: event_type.to_string(),
            payload_json: payload_json.to_string(),
            timestamp: timestamp.to_string(),
            stream_source: stream_source.to_string(),
            created_at: now,
        })
    }

    fn list_events(&self, session_id: &str, after_event_seq: Option<u64>) -> Result<Vec<EventRecord>> {
        let conn = self.conn.lock().expect("store db mutex poisoned");
        let (sql, params_vec): (&str, Vec<rusqlite::types::Value>) =
            if let Some(after) = after_event_seq {
                (
                    "SELECT id, session_id, event_seq, turn_seq, turn_id, event_type, payload_json, timestamp, stream_source, created_at
                     FROM biz_events
                     WHERE session_id = ?1 AND event_seq > ?2
                     ORDER BY event_seq ASC",
                    vec![
                        rusqlite::types::Value::from(session_id.to_string()),
                        rusqlite::types::Value::from(after as i64),
                    ],
                )
            } else {
                (
                    "SELECT id, session_id, event_seq, turn_seq, turn_id, event_type, payload_json, timestamp, stream_source, created_at
                     FROM biz_events
                     WHERE session_id = ?1
                     ORDER BY event_seq ASC",
                    vec![rusqlite::types::Value::from(session_id.to_string())],
                )
            };
        let mut stmt = conn
            .prepare(sql)
            .with_context(|| format!("prepare list events for session {}", session_id))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params_vec), |row| {
                Ok(EventRecord {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    event_seq: row.get::<_, i64>(2)? as u64,
                    turn_seq: row.get::<_, i64>(3)? as u64,
                    turn_id: row.get(4)?,
                    event_type: row.get(5)?,
                    payload_json: row.get(6)?,
                    timestamp: row.get(7)?,
                    stream_source: row.get(8)?,
                    created_at: row.get(9)?,
                })
            })
            .with_context(|| format!("query list events for session {}", session_id))?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row.context("read event row")?);
        }
        Ok(events)
    }

    fn last_event_seq(&self, session_id: &str) -> Result<Option<u64>> {
        let conn = self.conn.lock().expect("store db mutex poisoned");
        let seq = conn
            .query_row(
                "SELECT MAX(event_seq) FROM biz_events WHERE session_id = ?1",
                params![session_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()
            .context("query last event seq")?
            .flatten()
            .map(|v| v as u64);
        Ok(seq)
    }

    fn last_turn_seq_by_turn(&self, session_id: &str) -> Result<Vec<(String, u64)>> {
        let conn = self.conn.lock().expect("store db mutex poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT turn_id, MAX(turn_seq)
                 FROM biz_events
                 WHERE session_id = ?1 AND turn_id IS NOT NULL
                 GROUP BY turn_id",
            )
            .with_context(|| format!("prepare turn seq query for session {}", session_id))?;
        let rows = stmt
            .query_map(params![session_id], |row| {
                let turn_id: String = row.get(0)?;
                let turn_seq: i64 = row.get(1)?;
                Ok((turn_id, turn_seq as u64))
            })
            .with_context(|| format!("query turn seq for session {}", session_id))?;
        let mut items = Vec::new();
        for row in rows {
            items.push(row.context("read turn seq row")?);
        }
        Ok(items)
    }
}

impl ProviderRepository for SqliteStore {
    fn create_provider(
        &self,
        provider_name: &str,
        base_url: &str,
        model_name: &str,
        api_key: &str,
        provider_type: &str,
        context_window_size: u32,
    ) -> Result<ProviderRecord> {
        let provider_id = format!("provider_{}", Uuid::new_v4().simple());
        let now = now_rfc3339();
        {
            let conn = self.conn.lock().expect("store db mutex poisoned");
            conn.execute(
                "INSERT INTO llm_providers
                 (provider_id, provider_name, base_url, model_name, api_key, provider_type, context_window_size, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                params![provider_id, provider_name, base_url, model_name, api_key, provider_type, context_window_size, now],
            )
            .with_context(|| format!("insert provider {}", provider_name))?;
        }
        self.get_provider(&provider_id)?
            .context("provider must exist after create")
    }

    fn update_provider(
        &self,
        provider_id: &str,
        provider_name: &str,
        base_url: &str,
        model_name: &str,
        api_key: Option<&str>,
        context_window_size: u32,
    ) -> Result<Option<ProviderRecord>> {
        let now = now_rfc3339();
        let changed = {
            let conn = self.conn.lock().expect("store db mutex poisoned");
            let sql = if api_key.is_some() {
                "UPDATE llm_providers
                 SET provider_name = ?1, base_url = ?2, model_name = ?3, api_key = ?4,
                     context_window_size = ?5, updated_at = ?6
                 WHERE provider_id = ?7"
            } else {
                "UPDATE llm_providers
                 SET provider_name = ?1, base_url = ?2, model_name = ?3,
                     context_window_size = ?4, updated_at = ?5
                 WHERE provider_id = ?6"
            };
            if let Some(api_key) = api_key {
                conn.execute(
                    sql,
                    params![provider_name, base_url, model_name, api_key, context_window_size, now, provider_id],
                )
                .with_context(|| format!("update provider {}", provider_id))?
            } else {
                conn.execute(
                    sql,
                    params![provider_name, base_url, model_name, context_window_size, now, provider_id],
                )
                .with_context(|| format!("update provider {}", provider_id))?
            }
        };
        if changed == 0 {
            return Ok(None);
        }
        // sync active provider snapshot
        {
            let conn = self.conn.lock().expect("store db mutex poisoned");
            let now = now_rfc3339();
            conn.execute(
                "UPDATE llm_runtime_settings
                 SET model_name = ?1, context_window_size = ?2, updated_at = ?3
                 WHERE setting_key = 'active_provider' AND provider_id = ?4",
                params![model_name, context_window_size, now, provider_id],
            )
            .with_context(|| format!("sync active provider snapshot {}", provider_id))?;
        }
        self.get_provider(provider_id)
    }

    fn delete_provider(&self, provider_id: &str) -> Result<bool> {
        let changed = {
            let conn = self.conn.lock().expect("store db mutex poisoned");
            conn.execute(
                "DELETE FROM llm_providers WHERE provider_id = ?1",
                params![provider_id],
            )
            .with_context(|| format!("delete provider {}", provider_id))?
        };
        Ok(changed > 0)
    }

    fn get_provider(&self, provider_id: &str) -> Result<Option<ProviderRecord>> {
        let conn = self.conn.lock().expect("store db mutex poisoned");
        conn.query_row(
            "SELECT provider_id, provider_name, base_url, model_name, api_key,
                    provider_type, context_window_size, created_at, updated_at
             FROM llm_providers
             WHERE provider_id = ?1",
            params![provider_id],
            |row| {
                Ok(ProviderRecord {
                    provider_id: row.get(0)?,
                    provider_name: row.get(1)?,
                    base_url: row.get(2)?,
                    model_name: row.get(3)?,
                    api_key: row.get(4)?,
                    provider_type: row.get(5)?,
                    context_window_size: row.get::<_, u32>(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            },
        )
        .optional()
        .context("query provider by id")
    }

    fn list_providers(&self) -> Result<Vec<ProviderRecord>> {
        let conn = self.conn.lock().expect("store db mutex poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT provider_id, provider_name, base_url, model_name, api_key,
                        provider_type, context_window_size, created_at, updated_at
                 FROM llm_providers
                 ORDER BY created_at DESC",
            )
            .context("prepare list providers")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ProviderRecord {
                    provider_id: row.get(0)?,
                    provider_name: row.get(1)?,
                    base_url: row.get(2)?,
                    model_name: row.get(3)?,
                    api_key: row.get(4)?,
                    provider_type: row.get(5)?,
                    context_window_size: row.get::<_, u32>(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })
            .context("query list providers")?;

        let mut providers = Vec::new();
        for row in rows {
            providers.push(row.context("read provider row")?);
        }
        Ok(providers)
    }

    fn get_active_provider(&self) -> Result<Option<ActiveProviderRecord>> {
        let conn = self.conn.lock().expect("store db mutex poisoned");
        conn.query_row(
            "SELECT provider_id, model_name, context_window_size, updated_at
             FROM llm_runtime_settings
             WHERE setting_key = 'active_provider'
               AND provider_id IS NOT NULL
               AND model_name IS NOT NULL",
            [],
            |row| {
                Ok(ActiveProviderRecord {
                    provider_id: row.get(0)?,
                    model_name: row.get(1)?,
                    context_window_size: row.get::<_, u32>(2)?,
                    updated_at: row.get(3)?,
                })
            },
        )
        .optional()
        .context("query active provider")
    }

    fn set_active_provider(&self, provider_id: &str) -> Result<Option<ActiveProviderRecord>> {
        let provider = match self.get_provider(provider_id)? {
            Some(p) => p,
            None => return Ok(None),
        };
        let now = now_rfc3339();
        {
            let conn = self.conn.lock().expect("store db mutex poisoned");
            conn.execute(
                "INSERT INTO llm_runtime_settings
                 (setting_key, provider_id, model_name, context_window_size, updated_at)
                 VALUES ('active_provider', ?1, ?2, ?3, ?4)
                 ON CONFLICT(setting_key)
                 DO UPDATE SET provider_id = excluded.provider_id,
                               model_name = excluded.model_name,
                               context_window_size = excluded.context_window_size,
                               updated_at = excluded.updated_at",
                params![provider.provider_id, provider.model_name, provider.context_window_size, now],
            )
            .context("set active provider")?;
        }
        self.get_active_provider()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_store() -> SqliteStore {
        SqliteStore::open_memory().expect("open in-memory store")
    }

    #[test]
    fn can_create_session_and_append_messages() {
        let store = setup_store();
        let created = store
            .create_session("sess_test_1", Some("first session"))
            .expect("create session");
        assert_eq!(created.session_id, "sess_test_1");
        assert_eq!(created.title.as_deref(), Some("first session"));

        let msg1 = store
            .append_message("sess_test_1", Some("turn_1"), "user", "hello")
            .expect("append first message");
        let msg2 = store
            .append_message("sess_test_1", Some("turn_1"), "assistant", "world")
            .expect("append second message");

        assert_eq!(msg1.sequence, 1);
        assert_eq!(msg2.sequence, 2);

        let messages = store.list_messages("sess_test_1").expect("list messages");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");

        let sessions = store.list_sessions().expect("list sessions");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "sess_test_1");
    }

    #[test]
    fn can_append_and_list_events_in_sequence_order() {
        let store = setup_store();
        store
            .create_session("sess_evt_1", Some("events"))
            .expect("create session");
        store
            .append_event(
                "sess_evt_1", 1, 0, None, "user_message",
                r#"{"content":"hello"}"#, "2026-01-01T00:00:01Z", "synthetic",
            )
            .expect("append event 1");
        store
            .append_event(
                "sess_evt_1", 2, 1, Some("turn_1"), "response_started",
                "{}", "2026-01-01T00:00:02Z", "model_live",
            )
            .expect("append event 2");
        let all = store.list_events("sess_evt_1", None).expect("list all");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].event_seq, 1);
        assert_eq!(all[1].event_seq, 2);

        let tail = store
            .list_events("sess_evt_1", Some(1))
            .expect("list tail after seq 1");
        assert_eq!(tail.len(), 1);
        assert_eq!(tail[0].event_type, "response_started");
    }

    #[test]
    fn can_query_last_event_and_turn_sequences() {
        let store = setup_store();
        store.create_session("sess_evt_2", Some("events")).expect("create session");
        store.append_event("sess_evt_2", 10, 1, Some("turn_1"), "response_started", "{}", "2026-01-01T00:00:01Z", "model_live").expect("append 10");
        store.append_event("sess_evt_2", 11, 2, Some("turn_1"), "response_completed", r#"{"content":"ok"}"#, "2026-01-01T00:00:02Z", "model_live").expect("append 11");
        store.append_event("sess_evt_2", 12, 1, Some("turn_2"), "response_started", "{}", "2026-01-01T00:00:03Z", "model_live").expect("append 12");

        let last = store.last_event_seq("sess_evt_2").expect("last event seq");
        assert_eq!(last, Some(12));
        let turn_seq = store.last_turn_seq_by_turn("sess_evt_2").expect("turn seq by turn");
        assert!(turn_seq.contains(&(String::from("turn_1"), 2)));
        assert!(turn_seq.contains(&(String::from("turn_2"), 1)));
    }

    #[test]
    fn provider_name_has_unique_constraint() {
        let store = setup_store();
        store.create_provider("openai-main", "https://api.openai.com/v1", "gpt-4.1", "sk-1", "built_in", 128000).expect("create first");
        let dup = store.create_provider("openai-main", "https://api.openai.com/v1", "gpt-4.1-mini", "sk-2", "built_in", 128000);
        assert!(dup.is_err());
    }

    #[test]
    fn can_update_and_delete_provider() {
        let store = setup_store();
        let created = store.create_provider("glm-main", "https://open.bigmodel.cn/api/paas/v4", "glm-4", "key-a", "custom", 0).expect("create");
        let updated = store.update_provider(&created.provider_id, "glm-main", "https://open.bigmodel.cn/api/paas/v4", "glm-4-plus", Some("key-b"), 0).expect("update").expect("exists");
        assert_eq!(updated.model_name, "glm-4-plus");
        assert_eq!(updated.api_key, "key-b");

        let updated2 = store.update_provider(&created.provider_id, "glm-main-2", "https://open.bigmodel.cn/api/paas/v4", "glm-4-air", None, 0).expect("update no key").expect("exists");
        assert_eq!(updated2.provider_name, "glm-main-2");
        assert_eq!(updated2.api_key, "key-b");

        assert!(store.delete_provider(&created.provider_id).expect("delete"));
        assert!(store.get_provider(&created.provider_id).expect("get after delete").is_none());
    }

    #[test]
    fn can_set_and_get_active_provider() {
        let store = setup_store();
        let first = store.create_provider("openai-main", "https://api.openai.com/v1", "gpt-4.1-mini", "sk-1", "built_in", 128000).expect("create first");
        let second = store.create_provider("glm-main", "https://open.bigmodel.cn/api/paas/v4", "glm-4.7", "sk-2", "built_in", 128000).expect("create second");

        let selected = store.set_active_provider(&second.provider_id).expect("set active").expect("exists");
        assert_eq!(selected.provider_id, second.provider_id);
        assert_eq!(selected.model_name, "glm-4.7");

        let loaded = store.get_active_provider().expect("load active").expect("exists");
        assert_eq!(loaded.provider_id, second.provider_id);

        assert!(store.set_active_provider("provider_missing").expect("set missing").is_none());
        let unchanged = store.get_active_provider().expect("load after missing").expect("still exists");
        assert_eq!(unchanged.provider_id, second.provider_id);

        store.delete_provider(&second.provider_id).expect("delete active");
        assert!(store.get_active_provider().expect("load after delete").is_none());

        let reselect = store.set_active_provider(&first.provider_id).expect("reselect").expect("exists");
        assert_eq!(reselect.provider_id, first.provider_id);
    }

    #[test]
    fn provider_new_fields_roundtrip() {
        let store = setup_store();

        let p = store
            .create_provider("OpenAI", "https://api.openai.com/v1", "gpt-4o", "sk-test", "built_in", 128000)
            .expect("create provider");

        assert_eq!(p.provider_type, "built_in");
        assert_eq!(p.context_window_size, 128000);

        let updated = store
            .update_provider(&p.provider_id, "OpenAI", "https://api.openai.com/v1", "gpt-4o-mini", None, 128000)
            .expect("update provider")
            .expect("provider exists");

        assert_eq!(updated.model_name, "gpt-4o-mini");
        assert_eq!(updated.context_window_size, 128000);
        // provider_type is not mutated by update
        assert_eq!(updated.provider_type, "built_in");
    }

    #[test]
    fn active_provider_snapshot_includes_context_window() {
        let store = setup_store();
        let p = store
            .create_provider("Kimi", "https://api.kimi.com/coding", "k2.5", "key", "built_in", 256000)
            .expect("create");
        store.set_active_provider(&p.provider_id).expect("set active");

        let active = store.get_active_provider().expect("get active").expect("has active");
        assert_eq!(active.context_window_size, 256000);
    }

    #[test]
    fn active_snapshot_updated_when_provider_model_switches() {
        let store = setup_store();
        let p = store
            .create_provider("OpenAI", "https://api.openai.com/v1", "gpt-4o", "key", "built_in", 128000)
            .expect("create");
        store.set_active_provider(&p.provider_id).expect("set active");

        // switch model
        store
            .update_provider(&p.provider_id, "OpenAI", "https://api.openai.com/v1", "gpt-5.3-codex", None, 200000)
            .expect("update");

        let active = store.get_active_provider().expect("get").expect("active");
        assert_eq!(active.model_name, "gpt-5.3-codex");
        assert_eq!(active.context_window_size, 200000);
    }

    #[test]
    fn migration_is_idempotent() {
        let store = setup_store();
        let _ = store.create_provider("X", "https://x.com/v1", "m", "k", "custom", 0).expect("create");
        // Call migrate_schema twice on the same connection — must not error
        let conn = store.conn.lock().expect("lock");
        SqliteStore::migrate_schema(&conn).expect("first migrate ok");
        SqliteStore::migrate_schema(&conn).expect("second migrate ok (idempotent)");
    }
}
