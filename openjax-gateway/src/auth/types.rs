use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthScope {
    Owner,
}

impl AuthScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthSessionStatus {
    Active,
    Revoked,
    LoggedOut,
}

impl AuthSessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Revoked => "revoked",
            Self::LoggedOut => "logged_out",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionRecord {
    pub session_id: String,
    pub device_id: String,
    pub scope: String,
    pub device_name: Option<String>,
    pub platform: Option<String>,
    pub user_agent: Option<String>,
    pub status: String,
    pub created_at: String,
    pub last_seen_at: String,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LoginResult {
    pub access_token: String,
    pub access_expires_in: u64,
    pub refresh_token: String,
    pub session: SessionRecord,
}

#[derive(Debug, Clone)]
pub struct RefreshResult {
    pub access_token: String,
    pub access_expires_in: u64,
    pub refresh_token: String,
    pub session: SessionRecord,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionView {
    pub session_id: String,
    pub device_id: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    pub status: String,
    pub created_at: String,
    pub last_seen_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<String>,
}

impl From<SessionRecord> for SessionView {
    fn from(value: SessionRecord) -> Self {
        Self {
            session_id: value.session_id,
            device_id: value.device_id,
            scope: value.scope,
            device_name: value.device_name,
            platform: value.platform,
            user_agent: value.user_agent,
            status: value.status,
            created_at: value.created_at,
            last_seen_at: value.last_seen_at,
            revoked_at: value.revoked_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewSessionInput {
    pub device_name: Option<String>,
    pub platform: Option<String>,
    pub user_agent: Option<String>,
}
