use crate::auth::{self, create_provider};
use crate::config::{AuthMethod, PluginConfig};
use crate::error::Result;
use std::path::Path;

pub fn run(auth_dir: &Path) -> Result<()> {
    let method = auth::load_auth_meta(auth_dir)?;
    let config = minimal_config(&method);
    let provider = create_provider(&config);
    provider.check(auth_dir)
}

fn minimal_config(method: &AuthMethod) -> PluginConfig {
    let method_str = match method {
        AuthMethod::Pat => "pat",
        AuthMethod::Oauth => "oauth",
        AuthMethod::DeviceAuth => "device_auth",
    };
    serde_json::from_value(serde_json::json!({
        "url": "https://placeholder",
        "project": "X",
        "auth_method": method_str,
    }))
    .expect("minimal config")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::tokens::{self, PatCredentials};

    #[test]
    fn check_pat_ok() {
        let dir = tempfile::tempdir().unwrap();
        auth::save_auth_meta(dir.path(), &AuthMethod::Pat).unwrap();
        tokens::save_json(
            dir.path(),
            "credentials.json",
            &PatCredentials {
                username: "u".into(),
                token: "t".into(),
            },
        )
        .unwrap();
        run(dir.path()).unwrap();
    }

    #[test]
    fn check_no_meta() {
        let dir = tempfile::tempdir().unwrap();
        assert!(run(dir.path()).is_err());
    }

    #[test]
    fn check_pat_no_creds() {
        let dir = tempfile::tempdir().unwrap();
        auth::save_auth_meta(dir.path(), &AuthMethod::Pat).unwrap();
        assert!(run(dir.path()).is_err());
    }

    #[test]
    fn minimal_config_pat() {
        let cfg = minimal_config(&AuthMethod::Pat);
        assert_eq!(cfg.auth_method, AuthMethod::Pat);
    }

    #[test]
    fn minimal_config_oauth() {
        let cfg = minimal_config(&AuthMethod::Oauth);
        assert_eq!(cfg.auth_method, AuthMethod::Oauth);
    }

    #[test]
    fn minimal_config_oul() {
        let cfg = minimal_config(&AuthMethod::DeviceAuth);
        assert_eq!(cfg.auth_method, AuthMethod::DeviceAuth);
    }
}
