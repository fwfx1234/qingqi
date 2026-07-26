use std::collections::HashMap;
use std::sync::Mutex;

use uuid::Uuid;

/// Token store: maps token strings to device info.
pub struct TokenStore {
    tokens: Mutex<HashMap<String, TokenInfo>>,
}

#[derive(Clone, Debug)]
pub struct TokenInfo {
    pub device_name: String,
    pub created_at: i64,
    pub expires_at: i64,
}

impl TokenStore {
    pub fn new() -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
        }
    }

    /// Create a new token. Returns the token string.
    pub fn create_token(&self, device_name: &str, ttl_seconds: i64) -> String {
        let token = Uuid::new_v4().to_string();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let info = TokenInfo {
            device_name: device_name.to_string(),
            created_at: now,
            expires_at: now + ttl_seconds,
        };
        self.tokens.lock().unwrap().insert(token.clone(), info);
        token
    }

    /// Validate a token. Returns true if valid and not expired.
    pub fn validate(&self, token: &str) -> bool {
        let tokens = self.tokens.lock().unwrap();
        if let Some(info) = tokens.get(token) {
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            info.expires_at > now
        } else {
            false
        }
    }

    /// Revoke a token.
    pub fn revoke(&self, token: &str) -> bool {
        self.tokens.lock().unwrap().remove(token).is_some()
    }

    /// Clean up expired tokens.
    pub fn cleanup(&self) {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        self.tokens
            .lock()
            .unwrap()
            .retain(|_, info| info.expires_at > now);
    }

    /// List all active tokens.
    pub fn list_active(&self) -> Vec<(String, TokenInfo)> {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        self.tokens
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, info)| info.expires_at > now)
            .map(|(t, i)| (t.clone(), i.clone()))
            .collect()
    }
}
