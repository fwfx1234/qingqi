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
    pub permanent: bool,
}

impl TokenStore {
    pub fn new() -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
        }
    }

    /// Create a new token. Returns the token string.
    /// If permanent is true, the token never expires.
    pub fn create_token(&self, device_name: &str, ttl_seconds: i64) -> String {
        let permanent = ttl_seconds <= 0;
        let token = Uuid::new_v4().to_string();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let info = TokenInfo {
            device_name: device_name.to_string(),
            created_at: now,
            expires_at: if permanent { i64::MAX } else { now + ttl_seconds },
            permanent,
        };
        self.tokens.lock().unwrap().insert(token.clone(), info);
        token
    }

    /// Create a permanent token that never expires.
    pub fn create_permanent_token(&self, device_name: &str) -> String {
        self.create_token(device_name, 0)
    }

    /// Validate a token. Returns true if valid and not expired.
    /// Permanent tokens never expire.
    pub fn validate(&self, token: &str) -> bool {
        let tokens = self.tokens.lock().unwrap();
        if let Some(info) = tokens.get(token) {
            if info.permanent {
                return true;
            }
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

    /// Clean up expired tokens (permanent tokens are kept).
    pub fn cleanup(&self) {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        self.tokens
            .lock()
            .unwrap()
            .retain(|_, info| info.permanent || info.expires_at > now);
    }

    /// List all active tokens (including permanent ones).
    pub fn list_active(&self) -> Vec<(String, TokenInfo)> {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        self.tokens
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, info)| info.permanent || info.expires_at > now)
            .map(|(t, i)| (t.clone(), i.clone()))
            .collect()
    }

    /// Revoke a token by device name.
    pub fn revoke_by_name(&self, device_name: &str) -> bool {
        let mut tokens = self.tokens.lock().unwrap();
        let keys_to_remove: Vec<String> = tokens
            .iter()
            .filter(|(_, info)| info.device_name == device_name)
            .map(|(k, _)| k.clone())
            .collect();
        let removed = !keys_to_remove.is_empty();
        for key in keys_to_remove {
            tokens.remove(&key);
        }
        removed
    }
}
