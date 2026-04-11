pub mod oauth;
pub mod device_auth;
pub mod pat;
pub mod saml;
pub mod tokens;

use crate::config::{AuthMethod, PluginConfig};
use crate::error::Result;
use reqwest::blocking::RequestBuilder;
use std::path::Path;

/// Common interface for all auth strategies.
pub trait AuthProvider {
    /// Interactive setup: prompt user, store credentials in auth_dir.
    fn setup(&self, auth_dir: &Path) -> Result<()>;

    /// Non-interactive check: Ok(()) if credentials are valid.
    fn check(&self, auth_dir: &Path) -> Result<()>;

    /// Inject authentication into an outgoing HTTP request.
    fn authenticate(&self, auth_dir: &Path, builder: RequestBuilder) -> Result<RequestBuilder>;
}

pub fn create_provider(config: &PluginConfig) -> Box<dyn AuthProvider> {
    match config.auth_method {
        AuthMethod::Pat => Box::new(pat::PatAuth::new(config.clone())),
        AuthMethod::Oauth => Box::new(oauth::OAuthAuth::new(config.clone())),
        AuthMethod::DeviceAuth => Box::new(device_auth::DeviceAuth::new(config.clone())),
    }
}

/// Determine auth method from auth_dir metadata (for auth-check/auth-setup
/// which don't receive --config).
pub fn load_auth_meta(auth_dir: &Path) -> Result<AuthMethod> {
    let meta_path = auth_dir.join("auth_meta.json");
    let data = std::fs::read_to_string(&meta_path).map_err(|_| {
        crate::error::PluginError::Auth(
            "no auth configured. Run auth-setup first.".into(),
        )
    })?;
    let v: serde_json::Value = serde_json::from_str(&data)?;
    match v["auth_method"].as_str() {
        Some("pat") => Ok(AuthMethod::Pat),
        Some("oauth") => Ok(AuthMethod::Oauth),
        Some("device_auth") => Ok(AuthMethod::DeviceAuth),
        _ => Err(crate::error::PluginError::Auth(
            "unknown auth_method in auth_meta.json".into(),
        )),
    }
}

pub fn save_auth_meta(auth_dir: &Path, method: &AuthMethod) -> Result<()> {
    std::fs::create_dir_all(auth_dir)?;
    let method_str = match method {
        AuthMethod::Pat => "pat",
        AuthMethod::Oauth => "oauth",
        AuthMethod::DeviceAuth => "device_auth",
    };
    let meta = serde_json::json!({"auth_method": method_str});
    std::fs::write(
        auth_dir.join("auth_meta.json"),
        serde_json::to_string_pretty(&meta)?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(method: &str) -> PluginConfig {
        serde_json::from_str(&format!(
            r#"{{"url":"https://x","project":"X","auth_method":"{}"}}"#,
            method
        ))
        .unwrap()
    }

    #[test]
    fn create_provider_pat() {
        let p = create_provider(&make_config("pat"));
        // Just verify it doesn't panic
        let _ = p;
    }

    #[test]
    fn create_provider_oauth() {
        let p = create_provider(&make_config("oauth"));
        let _ = p;
    }

    #[test]
    fn create_provider_oul() {
        let p = create_provider(&make_config("device_auth"));
        let _ = p;
    }

    #[test]
    fn save_and_load_auth_meta() {
        let dir = tempfile::tempdir().unwrap();
        save_auth_meta(dir.path(), &AuthMethod::Pat).unwrap();
        assert_eq!(load_auth_meta(dir.path()).unwrap(), AuthMethod::Pat);

        save_auth_meta(dir.path(), &AuthMethod::Oauth).unwrap();
        assert_eq!(load_auth_meta(dir.path()).unwrap(), AuthMethod::Oauth);

        save_auth_meta(dir.path(), &AuthMethod::DeviceAuth).unwrap();
        assert_eq!(load_auth_meta(dir.path()).unwrap(), AuthMethod::DeviceAuth);
    }

    #[test]
    fn load_auth_meta_missing() {
        let dir = tempfile::tempdir().unwrap();
        let err = load_auth_meta(dir.path());
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("auth-setup"));
    }

    #[test]
    fn load_auth_meta_bad_method() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("auth_meta.json"),
            r#"{"auth_method":"magic"}"#,
        )
        .unwrap();
        let err = load_auth_meta(dir.path());
        assert!(err.is_err());
    }
}
