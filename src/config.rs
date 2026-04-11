use crate::error::{PluginError, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    Pat,
    Oauth,
    DeviceAuth,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerType {
    Cloud,
    Server,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginConfig {
    pub url: String,
    pub project: String,
    #[serde(default = "default_auth_method")]
    pub auth_method: AuthMethod,
    /// None means auto-detect from serverInfo API.
    #[serde(default)]
    pub server_type: Option<ServerType>,
    #[serde(default)]
    pub status_map: HashMap<String, String>,
    #[serde(default)]
    #[allow(dead_code)] // used for future field mapping expansion
    pub field_map: HashMap<String, String>,
    #[serde(default)]
    pub sync_filter: Option<String>,
    #[serde(default = "default_true")]
    pub create_in_remote: bool,
    #[serde(default = "default_true")]
    pub close_in_remote: bool,
    pub oauth_client_id: Option<String>,
    pub oauth_callback_port: Option<u16>,
    pub device_auth_helper_path: Option<String>,
}

impl PluginConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path).map_err(|e| {
            PluginError::Config(format!("{}: {}", path.display(), e))
        })?;
        serde_json::from_str(&data).map_err(|e| {
            PluginError::Config(format!("{}: {}", path.display(), e))
        })
    }

    pub fn effective_sync_filter(&self) -> String {
        self.sync_filter.clone().unwrap_or_else(|| {
            format!("project = {} AND status != Done", self.project)
        })
    }

    pub fn effective_server_type(&self) -> &ServerType {
        self.server_type.as_ref().unwrap_or(&ServerType::Cloud)
    }

    pub fn api_base(&self) -> &str {
        match self.effective_server_type() {
            ServerType::Cloud => "/rest/api/3",
            ServerType::Server => "/rest/api/2",
        }
    }

    /// True if server_type was not explicitly set (should auto-detect).
    pub fn server_type_auto(&self) -> bool {
        self.server_type.is_none()
    }
}

fn default_auth_method() -> AuthMethod {
    AuthMethod::Pat
}
fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_minimal() {
        let json = r#"{"url":"https://x.atlassian.net","project":"X"}"#;
        let cfg: PluginConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.url, "https://x.atlassian.net");
        assert_eq!(cfg.project, "X");
        assert_eq!(cfg.auth_method, AuthMethod::Pat);
        assert!(cfg.server_type.is_none()); // auto-detect
        assert_eq!(*cfg.effective_server_type(), ServerType::Cloud); // default
        assert!(cfg.server_type_auto());
        assert!(cfg.create_in_remote);
        assert!(cfg.close_in_remote);
        assert!(cfg.sync_filter.is_none());
    }

    #[test]
    fn deserialize_full() {
        let json = r#"{
            "url": "https://jira.example.com",
            "project": "PROJ",
            "auth_method": "device_auth",
            "server_type": "server",
            "status_map": {"open": "To Do"},
            "sync_filter": "project = PROJ AND assignee = me",
            "create_in_remote": false,
            "close_in_remote": false,
            "device_auth_helper_path": "/usr/local/bin/auth-helper"
        }"#;
        let cfg: PluginConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.auth_method, AuthMethod::DeviceAuth);
        assert_eq!(cfg.server_type, Some(ServerType::Server));
        assert!(!cfg.server_type_auto());
        assert!(!cfg.create_in_remote);
        assert_eq!(
            cfg.device_auth_helper_path.as_deref(),
            Some("/usr/local/bin/auth-helper")
        );
    }

    #[test]
    fn effective_sync_filter_default() {
        let json = r#"{"url":"https://x.net","project":"FOO"}"#;
        let cfg: PluginConfig = serde_json::from_str(json).unwrap();
        assert_eq!(
            cfg.effective_sync_filter(),
            "project = FOO AND status != Done"
        );
    }

    #[test]
    fn effective_sync_filter_custom() {
        let json = r#"{"url":"https://x.net","project":"FOO","sync_filter":"custom"}"#;
        let cfg: PluginConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.effective_sync_filter(), "custom");
    }

    #[test]
    fn api_base_cloud() {
        let json = r#"{"url":"https://x.net","project":"X","server_type":"cloud"}"#;
        let cfg: PluginConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.api_base(), "/rest/api/3");
    }

    #[test]
    fn api_base_server() {
        let json = r#"{"url":"https://x.net","project":"X","server_type":"server"}"#;
        let cfg: PluginConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.api_base(), "/rest/api/2");
    }

    #[test]
    fn load_missing_file() {
        let err = PluginConfig::load(Path::new("/nonexistent.json"));
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("/nonexistent.json"));
    }

    #[test]
    fn load_bad_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "not json").unwrap();
        let err = PluginConfig::load(&path);
        assert!(err.is_err());
    }

    #[test]
    fn load_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ok.json");
        std::fs::write(&path, r#"{"url":"https://x.net","project":"X"}"#).unwrap();
        let cfg = PluginConfig::load(&path).unwrap();
        assert_eq!(cfg.project, "X");
    }
}
