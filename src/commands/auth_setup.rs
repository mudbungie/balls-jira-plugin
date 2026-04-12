use crate::auth;
use crate::auth::pat::PatAuth;
use crate::config::{AuthMethod, PluginConfig};
use crate::error::{PluginError, Result};
use std::io::{self, BufRead, Write};
use std::path::Path;

pub fn run(auth_dir: &Path) -> Result<()> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    // Banner/prompts go to stderr so stdout remains clean for the plugin
    // JSON protocol when invoked by the balls core.
    let mut writer = io::stderr();
    let mut read_password = || -> Result<String> {
        rpassword::read_password()
            .map_err(|e| PluginError::Auth(format!("failed to read password: {}", e)))
    };
    run_with_io(auth_dir, &mut reader, &mut writer, &mut read_password)
}

pub fn run_with_io(
    auth_dir: &Path,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    read_password: &mut dyn FnMut() -> Result<String>,
) -> Result<()> {
    writeln!(output, "Jira Plugin Auth Setup")?;
    writeln!(output, "1) PAT (Personal Access Token)")?;
    writeln!(output, "2) OAuth 2.0 (Atlassian Cloud)")?;
    writeln!(output, "3) Device Auth (local SAML helper binary)")?;
    write!(output, "Choose auth method [1-3]: ")?;
    output.flush()?;

    let mut choice = String::new();
    input.read_line(&mut choice)?;
    let method = match choice.trim() {
        "1" | "pat" => AuthMethod::Pat,
        "2" | "oauth" => AuthMethod::Oauth,
        "3" | "device_auth" => AuthMethod::DeviceAuth,
        other => {
            return Err(PluginError::Auth(format!("Unknown auth method: {}", other)));
        }
    };

    write!(output, "Jira URL (e.g. https://company.atlassian.net): ")?;
    output.flush()?;
    let mut url = String::new();
    input.read_line(&mut url)?;
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err(PluginError::Config("Jira URL cannot be empty".into()));
    }

    write!(output, "Server type (cloud/server) [cloud]: ")?;
    output.flush()?;
    let mut server_type = String::new();
    input.read_line(&mut server_type)?;
    let server_type_str = if server_type.trim().is_empty() {
        "cloud"
    } else {
        server_type.trim()
    };

    let config = build_config(&url, &method, server_type_str)?;

    // PAT is interactive — thread IO through so tests can inject input.
    // OAuth and device_auth drive their own IO (browser, helper binary).
    match method {
        AuthMethod::Pat => {
            let pat = PatAuth::new(config);
            pat.setup_with_io(auth_dir, input, output, read_password)
        }
        _ => {
            let provider = auth::create_provider(&config);
            provider.setup(auth_dir)
        }
    }
}

fn build_config(url: &str, method: &AuthMethod, server_type: &str) -> Result<PluginConfig> {
    let method_str = match method {
        AuthMethod::Pat => "pat",
        AuthMethod::Oauth => "oauth",
        AuthMethod::DeviceAuth => "device_auth",
    };
    let json = serde_json::json!({
        "url": url,
        "project": "SETUP",
        "auth_method": method_str,
        "server_type": server_type,
    });
    serde_json::from_value(json).map_err(|e| PluginError::Config(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_token(token: &'static str) -> impl FnMut() -> Result<String> {
        move || Ok(token.to_string())
    }

    #[test]
    fn setup_pat_flow() {
        let dir = tempfile::tempdir().unwrap();
        let input_data = b"1\nhttps://jira.example.com\nserver\nadmin\n";
        let mut input = io::Cursor::new(input_data.as_slice());
        let mut output = Vec::new();
        let mut pw = with_token("my-token");
        run_with_io(dir.path(), &mut input, &mut output, &mut pw).unwrap();
        let out = String::from_utf8(output).unwrap();
        assert!(out.contains("PAT"));
        let meta = auth::load_auth_meta(dir.path()).unwrap();
        assert_eq!(meta, AuthMethod::Pat);
    }

    #[test]
    fn setup_invalid_choice() {
        let dir = tempfile::tempdir().unwrap();
        let mut input = io::Cursor::new(b"9\n");
        let mut output = Vec::new();
        let mut pw = with_token("");
        let err = run_with_io(dir.path(), &mut input, &mut output, &mut pw);
        assert!(err.is_err());
    }

    #[test]
    fn setup_empty_url() {
        let dir = tempfile::tempdir().unwrap();
        let mut input = io::Cursor::new(b"1\n\n");
        let mut output = Vec::new();
        let mut pw = with_token("");
        let err = run_with_io(dir.path(), &mut input, &mut output, &mut pw);
        assert!(err.is_err());
    }

    #[test]
    fn setup_default_server_type() {
        let dir = tempfile::tempdir().unwrap();
        let input_data = b"1\nhttps://x.atlassian.net\n\nuser@x.com\n";
        let mut input = io::Cursor::new(input_data.as_slice());
        let mut output = Vec::new();
        let mut pw = with_token("tok");
        run_with_io(dir.path(), &mut input, &mut output, &mut pw).unwrap();
    }

    #[test]
    fn build_config_all_methods() {
        build_config("https://x", &AuthMethod::Pat, "cloud").unwrap();
        build_config("https://x", &AuthMethod::Oauth, "server").unwrap();
        build_config("https://x", &AuthMethod::DeviceAuth, "server").unwrap();
    }
}
