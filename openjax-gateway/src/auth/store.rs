use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::auth::types::{AuthScope, AuthSessionStatus, SessionRecord};
use crate::error::now_rfc3339;

#[derive(Clone)]
pub struct AuthStore {
    conn: Arc<Mutex<Connection>>,
}

pub struct CreatedTokens {
    pub session: SessionRecord,
    pub access_expires_in: u64,
}

/// Parameters for creating a new session with tokens
pub struct CreateSessionParams<'a> {
    pub scope: AuthScope,
    pub device_name: Option<&'a str>,
    pub platform: Option<&'a str>,
    pub user_agent: Option<&'a str>,
    pub access_hash: &'a str,
    pub access_ttl: Duration,
    pub refresh_hash: &'a str,
    pub refresh_ttl: Duration,
}

impl AuthStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create auth db dir {}", parent.display()))?;
        }
        let conn =
            Connection::open(path).with_context(|| format!("open auth db {}", path.display()))?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("open in-memory auth db")?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().expect("auth db mutex poisoned");
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS auth_sessions (
                session_id TEXT PRIMARY KEY,
                device_id TEXT NOT NULL,
                scope TEXT NOT NULL,
                device_name TEXT,
                platform TEXT,
                user_agent TEXT,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_seen_at TEXT NOT NULL,
                revoked_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_auth_sessions_status ON auth_sessions(status);
            CREATE INDEX IF NOT EXISTS idx_auth_sessions_last_seen ON auth_sessions(last_seen_at);

            CREATE TABLE IF NOT EXISTS auth_refresh_tokens (
                token_id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                token_hash TEXT NOT NULL UNIQUE,
                rotated_from TEXT,
                expires_at TEXT NOT NULL,
                created_at TEXT NOT NULL,
                revoked_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_auth_refresh_session ON auth_refresh_tokens(session_id);
            CREATE INDEX IF NOT EXISTS idx_auth_refresh_expires ON auth_refresh_tokens(expires_at);

            CREATE TABLE IF NOT EXISTS auth_access_tokens (
                token_hash TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                created_at TEXT NOT NULL,
                revoked_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_auth_access_session ON auth_access_tokens(session_id);
            CREATE INDEX IF NOT EXISTS idx_auth_access_expires ON auth_access_tokens(expires_at);
            "#,
        )
        .context("init auth schema")?;
        Ok(())
    }

    pub fn create_session_and_tokens(
        &self,
        params: CreateSessionParams<'_>,
    ) -> Result<CreatedTokens> {
        let access_expires_in = params.access_ttl.whole_seconds().max(0) as u64;
        let session_id = format!("authsess_{}", Uuid::new_v4().simple());
        {
            let mut conn = self.conn.lock().expect("auth db mutex poisoned");
            let tx = conn.transaction().context("begin create auth tx")?;
            let now = OffsetDateTime::now_utc();
            let created_at = now_rfc3339();
            let access_expires_at = (now + params.access_ttl)
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_else(|_| created_at.clone());
            let refresh_expires_at = (now + params.refresh_ttl)
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_else(|_| created_at.clone());

            let device_id = format!("dev_{}", Uuid::new_v4().simple());
            tx.execute(
                "INSERT INTO auth_sessions (session_id, device_id, scope, device_name, platform, user_agent, status, created_at, last_seen_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                params![
                    session_id,
                    device_id,
                    params.scope.as_str(),
                    params.device_name,
                    params.platform,
                    params.user_agent,
                    AuthSessionStatus::Active.as_str(),
                    created_at,
                ],
            )
            .context("insert auth session")?;

            tx.execute(
                "INSERT INTO auth_access_tokens (token_hash, session_id, expires_at, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![params.access_hash, session_id, access_expires_at, created_at],
            )
            .context("insert auth access token")?;

            let refresh_token_id = format!("rtid_{}", Uuid::new_v4().simple());
            tx.execute(
                "INSERT INTO auth_refresh_tokens (token_id, session_id, token_hash, expires_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![refresh_token_id, session_id, params.refresh_hash, refresh_expires_at, created_at],
            )
            .context("insert auth refresh token")?;

            tx.commit().context("commit auth create tx")?;
        }
        let session = self
            .get_session(&session_id)?
            .context("session must exist after create")?;

        Ok(CreatedTokens {
            session,
            access_expires_in,
        })
    }

    pub fn validate_access_token(&self, token_hash: &str) -> Result<Option<SessionRecord>> {
        let session_id: Option<String> = {
            let conn = self.conn.lock().expect("auth db mutex poisoned");
            let now = now_rfc3339();
            conn.query_row(
                "SELECT session_id FROM auth_access_tokens WHERE token_hash = ?1 AND revoked_at IS NULL AND expires_at > ?2",
                params![token_hash, now],
                |row| row.get(0),
            )
            .optional()
            .context("query access token")?
        };
        match session_id {
            Some(id) => self.get_session(&id),
            None => Ok(None),
        }
    }

    pub fn rotate_refresh_token(
        &self,
        refresh_hash: &str,
        next_refresh_hash: &str,
        next_access_hash: &str,
        access_ttl: Duration,
        refresh_ttl: Duration,
    ) -> Result<RotateOutcome> {
        let access_expires_in_secs = access_ttl.whole_seconds().max(0) as u64;
        let session_id = {
            let mut conn = self.conn.lock().expect("auth db mutex poisoned");
            let tx = conn.transaction().context("begin refresh tx")?;
            let now = OffsetDateTime::now_utc();
            let now_str = now_rfc3339();

            let refresh_row: Option<(String, String, Option<String>, String)> = tx
                .query_row(
                    "SELECT token_id, session_id, revoked_at, expires_at FROM auth_refresh_tokens WHERE token_hash = ?1",
                    params![refresh_hash],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()
                .context("query refresh token")?;

            let (token_id, session_id, revoked_at, expires_at) = match refresh_row {
                Some(row) => row,
                None => {
                    tx.rollback().ok();
                    return Ok(RotateOutcome::Missing);
                }
            };

            if revoked_at.is_some() {
                tx.rollback().ok();
                return Ok(RotateOutcome::ReuseDetected);
            }

            if expires_at <= now_str {
                tx.execute(
                    "UPDATE auth_refresh_tokens SET revoked_at = ?1 WHERE token_id = ?2",
                    params![now_str, token_id],
                )
                .ok();
                tx.commit().ok();
                return Ok(RotateOutcome::Missing);
            }

            let session_status: Option<String> = tx
                .query_row(
                    "SELECT status FROM auth_sessions WHERE session_id = ?1",
                    params![session_id],
                    |row| row.get(0),
                )
                .optional()
                .context("query session status")?;

            if session_status.as_deref() != Some(AuthSessionStatus::Active.as_str()) {
                tx.rollback().ok();
                return Ok(RotateOutcome::Missing);
            }

            tx.execute(
                "UPDATE auth_refresh_tokens SET revoked_at = ?1 WHERE token_id = ?2",
                params![now_str, token_id],
            )
            .context("revoke old refresh token")?;

            let new_refresh_id = format!("rtid_{}", Uuid::new_v4().simple());
            let new_refresh_expires_at = (now + refresh_ttl)
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_else(|_| now_str.clone());
            tx.execute(
                "INSERT INTO auth_refresh_tokens (token_id, session_id, token_hash, rotated_from, expires_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    new_refresh_id,
                    session_id,
                    next_refresh_hash,
                    token_id,
                    new_refresh_expires_at,
                    now_str,
                ],
            )
            .context("insert rotated refresh token")?;

            let new_access_expires_at = (now + access_ttl)
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_else(|_| now_str.clone());
            tx.execute(
                "INSERT INTO auth_access_tokens (token_hash, session_id, expires_at, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![next_access_hash, session_id, new_access_expires_at, now_str],
            )
            .context("insert rotated access token")?;

            tx.execute(
                "UPDATE auth_sessions SET last_seen_at = ?1 WHERE session_id = ?2",
                params![now_str, session_id],
            )
            .context("touch session last_seen")?;

            tx.commit().context("commit refresh tx")?;
            session_id
        };

        let session = self
            .get_session(&session_id)?
            .context("session must exist after refresh")?;
        Ok(RotateOutcome::Rotated {
            session: Box::new(session),
            access_expires_in: access_expires_in_secs,
        })
    }

    pub fn revoke_sessions(
        &self,
        session_id: Option<&str>,
        device_id: Option<&str>,
        revoke_all: bool,
        status: AuthSessionStatus,
    ) -> Result<usize> {
        let mut conn = self.conn.lock().expect("auth db mutex poisoned");
        let tx = conn.transaction().context("begin revoke tx")?;
        let now = now_rfc3339();

        let session_ids: Vec<String> = {
            let mut ids = Vec::new();
            if revoke_all {
                let mut stmt = tx
                    .prepare("SELECT session_id FROM auth_sessions WHERE status = 'active'")
                    .context("prepare session ids query (all)")?;
                let rows = stmt
                    .query_map([], |row| row.get(0))
                    .context("map session ids (all)")?;
                for row in rows {
                    ids.push(row.context("read session id (all)")?);
                }
            } else if let (Some(sid), Some(did)) = (session_id, device_id) {
                let mut stmt = tx
                    .prepare(
                        "SELECT session_id FROM auth_sessions WHERE status = 'active' AND session_id = ?1 AND device_id = ?2",
                    )
                    .context("prepare session ids query (session + device)")?;
                let rows = stmt
                    .query_map(params![sid, did], |row| row.get(0))
                    .context("map session ids (session + device)")?;
                for row in rows {
                    ids.push(row.context("read session id (session + device)")?);
                }
            } else if let Some(sid) = session_id {
                let mut stmt = tx
                    .prepare(
                        "SELECT session_id FROM auth_sessions WHERE status = 'active' AND session_id = ?1",
                    )
                    .context("prepare session ids query (session)")?;
                let rows = stmt
                    .query_map(params![sid], |row| row.get(0))
                    .context("map session ids (session)")?;
                for row in rows {
                    ids.push(row.context("read session id (session)")?);
                }
            } else if let Some(did) = device_id {
                let mut stmt = tx
                    .prepare(
                        "SELECT session_id FROM auth_sessions WHERE status = 'active' AND device_id = ?1",
                    )
                    .context("prepare session ids query (device)")?;
                let rows = stmt
                    .query_map(params![did], |row| row.get(0))
                    .context("map session ids (device)")?;
                for row in rows {
                    ids.push(row.context("read session id (device)")?);
                }
            }
            ids
        };

        for sid in &session_ids {
            tx.execute(
                "UPDATE auth_sessions SET status = ?1, revoked_at = ?2 WHERE session_id = ?3",
                params![status.as_str(), now, sid],
            )
            .context("update session status")?;
            tx.execute(
                "UPDATE auth_refresh_tokens SET revoked_at = COALESCE(revoked_at, ?1) WHERE session_id = ?2",
                params![now, sid],
            )
            .context("revoke refresh by session")?;
            tx.execute(
                "UPDATE auth_access_tokens SET revoked_at = COALESCE(revoked_at, ?1) WHERE session_id = ?2",
                params![now, sid],
            )
            .context("revoke access by session")?;
        }

        tx.commit().context("commit revoke tx")?;
        Ok(session_ids.len())
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionRecord>> {
        let conn = self.conn.lock().expect("auth db mutex poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT session_id, device_id, scope, device_name, platform, user_agent, status, created_at, last_seen_at, revoked_at FROM auth_sessions WHERE status = 'active' ORDER BY created_at DESC",
            )
            .context("prepare list sessions")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(SessionRecord {
                    session_id: row.get(0)?,
                    device_id: row.get(1)?,
                    scope: row.get(2)?,
                    device_name: row.get(3)?,
                    platform: row.get(4)?,
                    user_agent: row.get(5)?,
                    status: row.get(6)?,
                    created_at: row.get(7)?,
                    last_seen_at: row.get(8)?,
                    revoked_at: row.get(9)?,
                })
            })
            .context("query list sessions")?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row.context("read session row")?);
        }
        Ok(sessions)
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionRecord>> {
        let conn = self.conn.lock().expect("auth db mutex poisoned");
        conn.query_row(
            "SELECT session_id, device_id, scope, device_name, platform, user_agent, status, created_at, last_seen_at, revoked_at FROM auth_sessions WHERE session_id = ?1",
            params![session_id],
            |row| {
                Ok(SessionRecord {
                    session_id: row.get(0)?,
                    device_id: row.get(1)?,
                    scope: row.get(2)?,
                    device_name: row.get(3)?,
                    platform: row.get(4)?,
                    user_agent: row.get(5)?,
                    status: row.get(6)?,
                    created_at: row.get(7)?,
                    last_seen_at: row.get(8)?,
                    revoked_at: row.get(9)?,
                })
            },
        )
        .optional()
        .context("query session by id")
    }

    pub fn cleanup_expired(&self) -> Result<()> {
        let conn = self.conn.lock().expect("auth db mutex poisoned");
        let now = now_rfc3339();
        conn.execute(
            "DELETE FROM auth_access_tokens WHERE expires_at <= ?1",
            params![now],
        )
        .context("cleanup access tokens")?;
        conn.execute(
            "DELETE FROM auth_refresh_tokens WHERE expires_at <= ?1 AND revoked_at IS NOT NULL",
            params![now],
        )
        .context("cleanup refresh tokens")?;
        Ok(())
    }
}

pub enum RotateOutcome {
    Missing,
    ReuseDetected,
    Rotated {
        session: Box<SessionRecord>,
        access_expires_in: u64,
    },
}
