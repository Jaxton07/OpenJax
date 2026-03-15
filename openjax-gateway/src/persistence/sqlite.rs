use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::error::now_rfc3339;
use crate::persistence::repository::{ProviderRepository, SessionRepository};
use crate::persistence::types::{
    ActiveProviderRecord, MessageRecord, ProviderRecord, SessionRecord,
};

#[derive(Clone)]
pub struct SqliteGatewayStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteGatewayStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create gateway db dir {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("open gateway db {}", path.display()))?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("open in-memory gateway db")?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().expect("gateway db mutex poisoned");
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
        .context("init gateway schema")?;
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

impl SessionRepository for SqliteGatewayStore {
    fn create_session(&self, session_id: &str, title: Option<&str>) -> Result<SessionRecord> {
        {
            let mut conn = self.conn.lock().expect("gateway db mutex poisoned");
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
        let conn = self.conn.lock().expect("gateway db mutex poisoned");
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
        let conn = self.conn.lock().expect("gateway db mutex poisoned");
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

    fn append_message(
        &self,
        session_id: &str,
        turn_id: Option<&str>,
        role: &str,
        content: &str,
    ) -> Result<MessageRecord> {
        let message_id = format!("msg_{}", Uuid::new_v4().simple());
        let mut conn = self.conn.lock().expect("gateway db mutex poisoned");
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
        let conn = self.conn.lock().expect("gateway db mutex poisoned");
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
}

impl ProviderRepository for SqliteGatewayStore {
    fn create_provider(
        &self,
        provider_name: &str,
        base_url: &str,
        model_name: &str,
        api_key: &str,
    ) -> Result<ProviderRecord> {
        let provider_id = format!("provider_{}", Uuid::new_v4().simple());
        let now = now_rfc3339();
        {
            let conn = self.conn.lock().expect("gateway db mutex poisoned");
            conn.execute(
                "INSERT INTO llm_providers (provider_id, provider_name, base_url, model_name, api_key, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                params![provider_id, provider_name, base_url, model_name, api_key, now],
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
    ) -> Result<Option<ProviderRecord>> {
        let now = now_rfc3339();
        let changed = {
            let conn = self.conn.lock().expect("gateway db mutex poisoned");
            let sql = if api_key.is_some() {
                "UPDATE llm_providers
                 SET provider_name = ?1, base_url = ?2, model_name = ?3, api_key = ?4, updated_at = ?5
                 WHERE provider_id = ?6"
            } else {
                "UPDATE llm_providers
                 SET provider_name = ?1, base_url = ?2, model_name = ?3, updated_at = ?4
                 WHERE provider_id = ?5"
            };
            if let Some(api_key) = api_key {
                conn.execute(
                    sql,
                    params![
                        provider_name,
                        base_url,
                        model_name,
                        api_key,
                        now,
                        provider_id
                    ],
                )
                .with_context(|| format!("update provider {}", provider_id))?
            } else {
                conn.execute(
                    sql,
                    params![provider_name, base_url, model_name, now, provider_id],
                )
                .with_context(|| format!("update provider {}", provider_id))?
            }
        };
        if changed == 0 {
            return Ok(None);
        }
        {
            let conn = self.conn.lock().expect("gateway db mutex poisoned");
            let now = now_rfc3339();
            conn.execute(
                "UPDATE llm_runtime_settings
                 SET model_name = ?1, updated_at = ?2
                 WHERE setting_key = 'active_provider'
                   AND provider_id = ?3",
                params![model_name, now, provider_id],
            )
            .with_context(|| format!("sync active provider model {}", provider_id))?;
        }
        self.get_provider(provider_id)
    }

    fn delete_provider(&self, provider_id: &str) -> Result<bool> {
        let changed = {
            let conn = self.conn.lock().expect("gateway db mutex poisoned");
            conn.execute(
                "DELETE FROM llm_providers WHERE provider_id = ?1",
                params![provider_id],
            )
            .with_context(|| format!("delete provider {}", provider_id))?
        };
        Ok(changed > 0)
    }

    fn get_provider(&self, provider_id: &str) -> Result<Option<ProviderRecord>> {
        let conn = self.conn.lock().expect("gateway db mutex poisoned");
        conn.query_row(
            "SELECT provider_id, provider_name, base_url, model_name, api_key, created_at, updated_at
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
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .optional()
        .context("query provider by id")
    }

    fn list_providers(&self) -> Result<Vec<ProviderRecord>> {
        let conn = self.conn.lock().expect("gateway db mutex poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT provider_id, provider_name, base_url, model_name, api_key, created_at, updated_at
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
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
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
        let conn = self.conn.lock().expect("gateway db mutex poisoned");
        conn.query_row(
            "SELECT provider_id, model_name, updated_at
             FROM llm_runtime_settings
             WHERE setting_key = 'active_provider'
               AND provider_id IS NOT NULL
               AND model_name IS NOT NULL",
            [],
            |row| {
                Ok(ActiveProviderRecord {
                    provider_id: row.get(0)?,
                    model_name: row.get(1)?,
                    updated_at: row.get(2)?,
                })
            },
        )
        .optional()
        .context("query active provider")
    }

    fn set_active_provider(&self, provider_id: &str) -> Result<Option<ActiveProviderRecord>> {
        let provider = match self.get_provider(provider_id)? {
            Some(provider) => provider,
            None => return Ok(None),
        };
        let now = now_rfc3339();
        {
            let conn = self.conn.lock().expect("gateway db mutex poisoned");
            conn.execute(
                "INSERT INTO llm_runtime_settings (setting_key, provider_id, model_name, updated_at)
                 VALUES ('active_provider', ?1, ?2, ?3)
                 ON CONFLICT(setting_key)
                 DO UPDATE SET provider_id = excluded.provider_id,
                               model_name = excluded.model_name,
                               updated_at = excluded.updated_at",
                params![provider.provider_id, provider.model_name, now],
            )
            .context("set active provider")?;
        }
        self.get_active_provider()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_store() -> SqliteGatewayStore {
        SqliteGatewayStore::open_memory().expect("open in-memory store")
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
    fn provider_name_has_unique_constraint() {
        let store = setup_store();
        let _ = store
            .create_provider(
                "openai-main",
                "https://api.openai.com/v1",
                "gpt-4.1",
                "sk-1",
            )
            .expect("create first provider");
        let duplicate = store.create_provider(
            "openai-main",
            "https://api.openai.com/v1",
            "gpt-4.1-mini",
            "sk-2",
        );
        assert!(duplicate.is_err());
    }

    #[test]
    fn can_update_and_delete_provider() {
        let store = setup_store();
        let created = store
            .create_provider(
                "glm-main",
                "https://open.bigmodel.cn/api/paas/v4",
                "glm-4",
                "key-a",
            )
            .expect("create provider");
        let updated = store
            .update_provider(
                &created.provider_id,
                "glm-main",
                "https://open.bigmodel.cn/api/paas/v4",
                "glm-4-plus",
                Some("key-b"),
            )
            .expect("update provider")
            .expect("provider exists");
        assert_eq!(updated.model_name, "glm-4-plus");
        assert_eq!(updated.api_key, "key-b");

        let updated_without_key = store
            .update_provider(
                &created.provider_id,
                "glm-main-2",
                "https://open.bigmodel.cn/api/paas/v4",
                "glm-4-air",
                None,
            )
            .expect("update provider without key")
            .expect("provider exists");
        assert_eq!(updated_without_key.provider_name, "glm-main-2");
        assert_eq!(updated_without_key.api_key, "key-b");

        let deleted = store
            .delete_provider(&created.provider_id)
            .expect("delete provider");
        assert!(deleted);
        let provider = store
            .get_provider(&created.provider_id)
            .expect("get provider after delete");
        assert!(provider.is_none());
    }

    #[test]
    fn can_set_and_get_active_provider() {
        let store = setup_store();
        let first = store
            .create_provider(
                "openai-main",
                "https://api.openai.com/v1",
                "gpt-4.1-mini",
                "sk-1",
            )
            .expect("create first provider");
        let second = store
            .create_provider(
                "glm-main",
                "https://open.bigmodel.cn/api/paas/v4",
                "glm-4.7",
                "sk-2",
            )
            .expect("create second provider");

        let selected = store
            .set_active_provider(&second.provider_id)
            .expect("set active provider")
            .expect("active provider exists");
        assert_eq!(selected.provider_id, second.provider_id);
        assert_eq!(selected.model_name, "glm-4.7");

        let loaded = store
            .get_active_provider()
            .expect("load active provider")
            .expect("active provider exists");
        assert_eq!(loaded.provider_id, second.provider_id);

        let missing = store
            .set_active_provider("provider_missing")
            .expect("set missing provider");
        assert!(missing.is_none());

        let unchanged = store
            .get_active_provider()
            .expect("load active provider after missing")
            .expect("active provider still exists");
        assert_eq!(unchanged.provider_id, second.provider_id);

        store
            .delete_provider(&second.provider_id)
            .expect("delete active provider");
        let cleared = store
            .get_active_provider()
            .expect("load active provider after delete");
        assert!(cleared.is_none());

        let selected_first = store
            .set_active_provider(&first.provider_id)
            .expect("reselect provider")
            .expect("active provider exists");
        assert_eq!(selected_first.provider_id, first.provider_id);
    }
}
