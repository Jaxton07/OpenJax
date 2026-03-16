use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration as StdDuration;

use anyhow::{Context, Result as AnyResult};
use openjax_core::OpenJaxPaths;
use time::Duration;

use crate::auth::rate_limit::SlidingWindowRateLimiter;
use crate::auth::store::{AuthStore, RotateOutcome, CreateSessionParams};
use crate::auth::token::{generate_token, hash_token};
use crate::auth::types::{
    AuthScope, AuthSessionStatus, LoginResult, NewSessionInput, RefreshResult, SessionRecord,
    SessionView,
};

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub db_path: PathBuf,
    pub access_ttl_minutes: i64,
    pub refresh_ttl_days: i64,
    pub cookie_secure: bool,
    pub login_rate_limit_per_min: usize,
    pub refresh_rate_limit_per_min: usize,
    pub token_pepper: String,
}

impl AuthConfig {
    pub fn from_env() -> Self {
        let db_path = OpenJaxPaths::detect()
            .map(|paths| {
                let _ = paths.ensure_runtime_dirs();
                paths.database_dir.join("auth.db")
            })
            .unwrap_or_else(|| PathBuf::from(".openjax/database/auth.db"));
        let access_ttl_minutes = std::env::var("OPENJAX_GATEWAY_ACCESS_TTL_MINUTES")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(15);
        let refresh_ttl_days = std::env::var("OPENJAX_GATEWAY_REFRESH_TTL_DAYS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(30);
        let cookie_secure = std::env::var("OPENJAX_GATEWAY_COOKIE_SECURE")
            .ok()
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
            .unwrap_or(true);
        let login_rate_limit_per_min =
            std::env::var("OPENJAX_GATEWAY_AUTH_RATE_LIMIT_LOGIN_PER_MIN")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(30);
        let refresh_rate_limit_per_min =
            std::env::var("OPENJAX_GATEWAY_AUTH_RATE_LIMIT_REFRESH_PER_MIN")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(120);
        let token_pepper = std::env::var("OPENJAX_GATEWAY_AUTH_TOKEN_PEPPER")
            .unwrap_or_else(|_| "openjax-local-dev-pepper".to_string());
        Self {
            db_path,
            access_ttl_minutes,
            refresh_ttl_days,
            cookie_secure,
            login_rate_limit_per_min,
            refresh_rate_limit_per_min,
            token_pepper,
        }
    }

    pub fn access_ttl(&self) -> Duration {
        Duration::minutes(self.access_ttl_minutes)
    }

    pub fn refresh_ttl(&self) -> Duration {
        Duration::days(self.refresh_ttl_days)
    }

    pub fn refresh_ttl_seconds(&self) -> i64 {
        self.refresh_ttl().whole_seconds()
    }
}

#[derive(Debug)]
pub enum RefreshError {
    Missing,
    ReuseDetected,
}

#[derive(Clone)]
pub struct AuthService {
    config: AuthConfig,
    store: AuthStore,
    login_limiter: std::sync::Arc<Mutex<SlidingWindowRateLimiter>>,
    refresh_limiter: std::sync::Arc<Mutex<SlidingWindowRateLimiter>>,
}

impl AuthService {
    pub fn from_config(config: AuthConfig) -> AnyResult<Self> {
        let store = AuthStore::open(&config.db_path)?;
        Ok(Self {
            login_limiter: std::sync::Arc::new(Mutex::new(SlidingWindowRateLimiter::new(
                config.login_rate_limit_per_min,
                StdDuration::from_secs(60),
            ))),
            refresh_limiter: std::sync::Arc::new(Mutex::new(SlidingWindowRateLimiter::new(
                config.refresh_rate_limit_per_min,
                StdDuration::from_secs(60),
            ))),
            config,
            store,
        })
    }

    pub fn for_test() -> AnyResult<Self> {
        let config = AuthConfig {
            db_path: PathBuf::from(":memory:"),
            access_ttl_minutes: 15,
            refresh_ttl_days: 30,
            cookie_secure: false,
            login_rate_limit_per_min: 1000,
            refresh_rate_limit_per_min: 1000,
            token_pepper: "test-pepper".to_string(),
        };
        let store = AuthStore::open_memory()?;
        Ok(Self {
            login_limiter: std::sync::Arc::new(Mutex::new(SlidingWindowRateLimiter::new(
                config.login_rate_limit_per_min,
                StdDuration::from_secs(60),
            ))),
            refresh_limiter: std::sync::Arc::new(Mutex::new(SlidingWindowRateLimiter::new(
                config.refresh_rate_limit_per_min,
                StdDuration::from_secs(60),
            ))),
            config,
            store,
        })
    }

    pub fn config(&self) -> &AuthConfig {
        &self.config
    }

    pub fn allow_login(&self, key: &str) -> bool {
        let mut limiter = self.login_limiter.lock().expect("login limiter mutex");
        limiter.allow(key)
    }

    pub fn allow_refresh(&self, key: &str) -> bool {
        let mut limiter = self.refresh_limiter.lock().expect("refresh limiter mutex");
        limiter.allow(key)
    }

    pub fn login(&self, input: NewSessionInput) -> AnyResult<LoginResult> {
        self.store.cleanup_expired().ok();
        let access_token = generate_token("atk");
        let refresh_token = generate_token("rtk");
        let access_hash = hash_token(&access_token, &self.config.token_pepper);
        let refresh_hash = hash_token(&refresh_token, &self.config.token_pepper);

        let created = self
            .store
            .create_session_and_tokens(CreateSessionParams {
                scope: AuthScope::Owner,
                device_name: input.device_name.as_deref(),
                platform: input.platform.as_deref(),
                user_agent: input.user_agent.as_deref(),
                access_hash: &access_hash,
                access_ttl: self.config.access_ttl(),
                refresh_hash: &refresh_hash,
                refresh_ttl: self.config.refresh_ttl(),
            })
            .context("create session and tokens")?;

        Ok(LoginResult {
            access_token,
            access_expires_in: created.access_expires_in,
            refresh_token,
            session: created.session,
        })
    }

    pub fn refresh(&self, refresh_token: &str) -> std::result::Result<RefreshResult, RefreshError> {
        self.store.cleanup_expired().ok();
        let current_hash = hash_token(refresh_token, &self.config.token_pepper);
        let next_refresh_token = generate_token("rtk");
        let next_access_token = generate_token("atk");
        let next_refresh_hash = hash_token(&next_refresh_token, &self.config.token_pepper);
        let next_access_hash = hash_token(&next_access_token, &self.config.token_pepper);

        match self.store.rotate_refresh_token(
            &current_hash,
            &next_refresh_hash,
            &next_access_hash,
            self.config.access_ttl(),
            self.config.refresh_ttl(),
        ) {
            Ok(RotateOutcome::Missing) => Err(RefreshError::Missing),
            Ok(RotateOutcome::ReuseDetected) => Err(RefreshError::ReuseDetected),
            Ok(RotateOutcome::Rotated {
                session,
                access_expires_in,
            }) => Ok(RefreshResult {
                access_token: next_access_token,
                access_expires_in,
                refresh_token: next_refresh_token,
                session: *session,
            }),
            Err(_) => Err(RefreshError::Missing),
        }
    }

    pub fn validate_access_token(&self, access_token: &str) -> Option<SessionRecord> {
        let access_hash = hash_token(access_token, &self.config.token_pepper);
        self.store
            .validate_access_token(&access_hash)
            .ok()
            .flatten()
    }

    pub fn logout_by_session(&self, session_id: &str) -> AnyResult<usize> {
        self.store
            .revoke_sessions(Some(session_id), None, false, AuthSessionStatus::LoggedOut)
            .context("logout by session")
    }

    pub fn revoke(
        &self,
        session_id: Option<&str>,
        device_id: Option<&str>,
        revoke_all: bool,
    ) -> AnyResult<usize> {
        self.store
            .revoke_sessions(
                session_id,
                device_id,
                revoke_all,
                AuthSessionStatus::Revoked,
            )
            .context("revoke sessions")
    }

    pub fn list_sessions(&self) -> AnyResult<Vec<SessionView>> {
        let sessions = self.store.list_sessions().context("list sessions")?;
        Ok(sessions.into_iter().map(Into::into).collect())
    }
}
