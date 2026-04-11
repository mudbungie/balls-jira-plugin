use super::tokens::{self, PatCredentials};
use super::AuthProvider;
use crate::config::{PluginConfig, ServerType};
use crate::error::{PluginError, Result};
use base64::Engine;
use reqwest::blocking::RequestBuilder;
use std::io::{self, BufRead, Write};
use std::path::Path;

pub struct PatAuth {
    config: PluginConfig,
}

impl PatAuth {
    pub fn new(config: PluginConfig) -> Self {
        Self { config }
    }

    pub fn setup_with_io(
        &self,
        auth_dir: &Path,
        input: &mut dyn BufRead,
        output: &mut dyn Write,
    ) -> Result<()> {
        let prompt = match self.config.server_type {
            ServerType::Cloud => "Email address",
            ServerType::Server => "Username",
        };
        write!(output, "{}: ", prompt)?;
        output.flush()?;
        let mut username = String::new();
        input.read_line(&mut username)?;
        let username = username.trim().to_string();
        if username.is_empty() {
            return Err(PluginError::Auth("username cannot be empty".into()));
        }

        write!(output, "API token: ")?;
        output.flush()?;
        let mut token = String::new();
        input.read_line(&mut token)?;
        let token = token.trim().to_string();
        if token.is_empty() {
            return Err(PluginError::Auth("token cannot be empty".into()));
        }

        let creds = PatCredentials { username, token };
        tokens::save_json(auth_dir, "credentials.json", &creds)?;
        super::save_auth_meta(auth_dir, &crate::config::AuthMethod::Pat)?;
        writeln!(output, "PAT credentials saved.")?;
        Ok(())
    }
}

impl AuthProvider for PatAuth {
    fn setup(&self, auth_dir: &Path) -> Result<()> {
        let stdin = io::stdin();
        let mut reader = stdin.lock();
        let mut writer = io::stderr();
        self.setup_with_io(auth_dir, &mut reader, &mut writer)
    }

    fn check(&self, auth_dir: &Path) -> Result<()> {
        let _creds: PatCredentials = tokens::load_json(auth_dir, "credentials.json")?;
        Ok(())
    }

    fn authenticate(&self, auth_dir: &Path, builder: RequestBuilder) -> Result<RequestBuilder> {
        let creds: PatCredentials = tokens::load_json(auth_dir, "credentials.json")?;
        let encoded = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", creds.username, creds.token));
        Ok(builder.header("Authorization", format!("Basic {}", encoded)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cloud_config() -> PluginConfig {
        serde_json::from_str(
            r#"{"url":"https://x.atlassian.net","project":"X","server_type":"cloud"}"#,
        )
        .unwrap()
    }

    fn server_config() -> PluginConfig {
        serde_json::from_str(
            r#"{"url":"https://jira.example.com","project":"X","server_type":"server"}"#,
        )
        .unwrap()
    }

    #[test]
    fn setup_cloud_prompts_email() {
        let auth = PatAuth::new(cloud_config());
        let dir = tempfile::tempdir().unwrap();
        let mut input = io::Cursor::new(b"user@example.com\nsecret-token\n");
        let mut output = Vec::new();
        auth.setup_with_io(dir.path(), &mut input, &mut output)
            .unwrap();
        let out = String::from_utf8(output).unwrap();
        assert!(out.contains("Email"));
        let creds: PatCredentials = tokens::load_json(dir.path(), "credentials.json").unwrap();
        assert_eq!(creds.username, "user@example.com");
        assert_eq!(creds.token, "secret-token");
    }

    #[test]
    fn setup_server_prompts_username() {
        let auth = PatAuth::new(server_config());
        let dir = tempfile::tempdir().unwrap();
        let mut input = io::Cursor::new(b"admin\nmy-pat\n");
        let mut output = Vec::new();
        auth.setup_with_io(dir.path(), &mut input, &mut output)
            .unwrap();
        let out = String::from_utf8(output).unwrap();
        assert!(out.contains("Username"));
    }

    #[test]
    fn setup_empty_username_fails() {
        let auth = PatAuth::new(cloud_config());
        let dir = tempfile::tempdir().unwrap();
        let mut input = io::Cursor::new(b"\nsecret\n");
        let mut output = Vec::new();
        let err = auth.setup_with_io(dir.path(), &mut input, &mut output);
        assert!(err.is_err());
    }

    #[test]
    fn setup_empty_token_fails() {
        let auth = PatAuth::new(cloud_config());
        let dir = tempfile::tempdir().unwrap();
        let mut input = io::Cursor::new(b"user@x.com\n\n");
        let mut output = Vec::new();
        let err = auth.setup_with_io(dir.path(), &mut input, &mut output);
        assert!(err.is_err());
    }

    #[test]
    fn check_succeeds_with_creds() {
        let auth = PatAuth::new(cloud_config());
        let dir = tempfile::tempdir().unwrap();
        tokens::save_json(
            dir.path(),
            "credentials.json",
            &PatCredentials {
                username: "u".into(),
                token: "t".into(),
            },
        )
        .unwrap();
        auth.check(dir.path()).unwrap();
    }

    #[test]
    fn check_fails_without_creds() {
        let auth = PatAuth::new(cloud_config());
        let dir = tempfile::tempdir().unwrap();
        assert!(auth.check(dir.path()).is_err());
    }

    #[test]
    fn authenticate_adds_basic_header() {
        let auth = PatAuth::new(cloud_config());
        let dir = tempfile::tempdir().unwrap();
        tokens::save_json(
            dir.path(),
            "credentials.json",
            &PatCredentials {
                username: "user@x.com".into(),
                token: "tok".into(),
            },
        )
        .unwrap();
        let client = reqwest::blocking::Client::new();
        let builder = client.get("https://example.com");
        let builder = auth.authenticate(dir.path(), builder).unwrap();
        let req = builder.build().unwrap();
        let header = req.headers().get("Authorization").unwrap().to_str().unwrap();
        assert!(header.starts_with("Basic "));
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(header.strip_prefix("Basic ").unwrap())
            .unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), "user@x.com:tok");
    }
}
