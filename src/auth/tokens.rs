use crate::error::{PluginError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatCredentials {
    pub username: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
    pub cloud_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSession {
    pub cookies: Vec<SessionCookie>,
    pub obtained_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCookie {
    pub name: String,
    pub value: String,
}

pub fn load_json<T: serde::de::DeserializeOwned>(auth_dir: &Path, filename: &str) -> Result<T> {
    let path = auth_dir.join(filename);
    let data = std::fs::read_to_string(&path).map_err(|e| {
        PluginError::Auth(format!("{}: {}", path.display(), e))
    })?;
    serde_json::from_str(&data).map_err(|e| {
        PluginError::Auth(format!("{}: {}", path.display(), e))
    })
}

pub fn save_json<T: serde::Serialize>(auth_dir: &Path, filename: &str, value: &T) -> Result<()> {
    std::fs::create_dir_all(auth_dir)?;
    let path = auth_dir.join(filename);
    let data = serde_json::to_string_pretty(value)?;
    std::fs::write(&path, data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_pat_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let creds = PatCredentials {
            username: "user@example.com".into(),
            token: "secret".into(),
        };
        save_json(dir.path(), "creds.json", &creds).unwrap();
        let loaded: PatCredentials = load_json(dir.path(), "creds.json").unwrap();
        assert_eq!(loaded.username, "user@example.com");
        assert_eq!(loaded.token, "secret");
    }

    #[test]
    fn roundtrip_oauth_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let tokens = OAuthTokens {
            access_token: "acc".into(),
            refresh_token: "ref".into(),
            expires_at: Utc::now(),
            cloud_id: "cloud-123".into(),
        };
        save_json(dir.path(), "oauth.json", &tokens).unwrap();
        let loaded: OAuthTokens = load_json(dir.path(), "oauth.json").unwrap();
        assert_eq!(loaded.access_token, "acc");
        assert_eq!(loaded.cloud_id, "cloud-123");
    }

    #[test]
    fn roundtrip_device_session() {
        let dir = tempfile::tempdir().unwrap();
        let session = DeviceSession {
            cookies: vec![SessionCookie {
                name: "JSESSIONID".into(),
                value: "abc123".into(),
            }],
            obtained_at: Utc::now(),
        };
        save_json(dir.path(), "session.json", &session).unwrap();
        let loaded: DeviceSession = load_json(dir.path(), "session.json").unwrap();
        assert_eq!(loaded.cookies.len(), 1);
        assert_eq!(loaded.cookies[0].name, "JSESSIONID");
    }

    #[test]
    fn load_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let err = load_json::<PatCredentials>(dir.path(), "nope.json");
        assert!(err.is_err());
    }

    #[test]
    fn load_bad_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("bad.json"), "not json").unwrap();
        let err = load_json::<PatCredentials>(dir.path(), "bad.json");
        assert!(err.is_err());
    }

    #[test]
    fn save_creates_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a/b/c");
        save_json(&nested, "test.json", &"hello").unwrap();
        let loaded: String = load_json(&nested, "test.json").unwrap();
        assert_eq!(loaded, "hello");
    }
}
