use crate::auth;
use crate::auth::pat::PatAuth;
use crate::config::{AuthMethod, PluginConfig};
use crate::error::{PluginError, Result};
use std::io::{self, BufRead, Write};
use std::path::Path;

pub fn run(auth_dir: &Path) -> Result<()> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut writer = io::stderr();
    run_with_io(auth_dir, &mut reader, &mut writer)
}

pub fn run_with_io(
    auth_dir: &Path,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Result<()> {
    writeln!(output, "Jira Plugin Auth Setup")?;
    writeln!(output, "1) PAT (Personal Access Token)")?;
    writeln!(output, "2) OAuth 2.0 (Atlassian Cloud)")?;
    writeln!(output, "3) device auth(SAML device helper)")?;
    write!(output, "Choose auth method [1-3]: ")?;
    output.flush()?;

    let mut choice = String::new();
    input.read_line(&mut choice)?;
    let method = match choice.trim() {
        "1" | "pat" => AuthMethod::Pat,
        "2" | "oauth" => AuthMethod::Oauth,
        "3" | "device_auth" => AuthMethod::DeviceAuth,
        other => {
            return Err(PluginError::Auth(format!("unknown auth method: {}", other)));
        }
    };

    write!(output, "Jira URL (e.g. https://company.atlassian.net): ")?;
    output.flush()?;
    let mut url = String::new();
    input.read_line(&mut url)?;
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err(PluginError::Config("URL cannot be empty".into()));
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

    // For PAT, we can route IO through directly.
    // For OAuth and device auth, they use their own IO (browser, device authbinary).
    match method {
        AuthMethod::Pat => {
            let pat = PatAuth::new(config);
            pat.setup_with_io(auth_dir, input, output)
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

    #[test]
    fn setup_pat_flow() {
        let dir = tempfile::tempdir().unwrap();
        let input_data = b"1\nhttps://jira.example.com\nserver\nadmin\nmy-token\n";
        let mut input = io::Cursor::new(input_data.as_slice());
        let mut output = Vec::new();
        run_with_io(dir.path(), &mut input, &mut output).unwrap();
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
        let err = run_with_io(dir.path(), &mut input, &mut output);
        assert!(err.is_err());
    }

    #[test]
    fn setup_empty_url() {
        let dir = tempfile::tempdir().unwrap();
        let mut input = io::Cursor::new(b"1\n\n");
        let mut output = Vec::new();
        let err = run_with_io(dir.path(), &mut input, &mut output);
        assert!(err.is_err());
    }

    #[test]
    fn setup_default_server_type() {
        let dir = tempfile::tempdir().unwrap();
        let input_data = b"1\nhttps://x.atlassian.net\n\nuser@x.com\ntok\n";
        let mut input = io::Cursor::new(input_data.as_slice());
        let mut output = Vec::new();
        run_with_io(dir.path(), &mut input, &mut output).unwrap();
    }

    #[test]
    fn build_config_all_methods() {
        build_config("https://x", &AuthMethod::Pat, "cloud").unwrap();
        build_config("https://x", &AuthMethod::Oauth, "server").unwrap();
        build_config("https://x", &AuthMethod::DeviceAuth, "server").unwrap();
    }
}
