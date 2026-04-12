use super::oauth_flow;
use super::tokens::{self, OAuthTokens};
use super::AuthProvider;
use crate::config::PluginConfig;
use crate::error::{PluginError, Result};
use chrono::{Duration, Utc};
use reqwest::blocking::RequestBuilder;
use std::path::Path;

pub struct OAuthAuth {
    config: PluginConfig,
}

impl OAuthAuth {
    pub fn new(config: PluginConfig) -> Self {
        Self { config }
    }

    fn client_id(&self) -> Result<&str> {
        self.config
            .oauth_client_id
            .as_deref()
            .ok_or_else(|| PluginError::Config("oauth_client_id required for OAuth".into()))
    }

    fn refresh_tokens(&self, auth_dir: &Path, tokens: &OAuthTokens) -> Result<OAuthTokens> {
        let client = reqwest::blocking::Client::new();
        let resp = client
            .post("https://auth.atlassian.com/oauth/token")
            .json(&serde_json::json!({
                "grant_type": "refresh_token",
                "client_id": self.client_id()?,
                "refresh_token": tokens.refresh_token,
            }))
            .send()?;
        if !resp.status().is_success() {
            return Err(PluginError::Auth(format!(
                "Token refresh failed: HTTP {}",
                resp.status()
            )));
        }
        let body: serde_json::Value = resp.json()?;
        let new_tokens = OAuthTokens {
            access_token: body["access_token"]
                .as_str()
                .ok_or_else(|| PluginError::Auth("No access_token in refresh response".into()))?
                .to_string(),
            refresh_token: body["refresh_token"]
                .as_str()
                .unwrap_or(&tokens.refresh_token)
                .to_string(),
            expires_at: Utc::now()
                + Duration::seconds(body["expires_in"].as_i64().unwrap_or(3600)),
            cloud_id: tokens.cloud_id.clone(),
        };
        tokens::save_json(auth_dir, "oauth_tokens.json", &new_tokens)?;
        Ok(new_tokens)
    }
}

impl AuthProvider for OAuthAuth {
    fn setup(&self, auth_dir: &Path) -> Result<()> {
        let client_id = self.client_id()?;
        let port = self.config.oauth_callback_port.unwrap_or(19472);
        let (verifier, challenge) = oauth_flow::generate_pkce();
        let state = oauth_flow::generate_state();
        let url = oauth_flow::build_authorize_url(client_id, port, &challenge, &state);

        eprintln!("Opening browser for Atlassian OAuth...");
        eprintln!("If the browser doesn't open, visit:\n{}", url);
        if let Err(e) = open::that(&url) {
            eprintln!("Warning: failed to open browser ({}). Open the URL manually.", e);
        }

        let server = tiny_http::Server::http(format!("127.0.0.1:{}", port))
            .map_err(|e| PluginError::Auth(format!("Callback server: {}", e)))?;
        eprintln!("Waiting for OAuth callback on port {}...", port);

        let code = loop {
            let request = server
                .recv()
                .map_err(|e| PluginError::Auth(format!("Callback recv: {}", e)))?;
            let request_url = request.url().to_string();
            match oauth_flow::parse_callback(&request_url, &state) {
                Ok(code) => {
                    oauth_flow::respond_ok(request);
                    break code;
                }
                Err(_) if !request_url.starts_with("/callback") => {
                    // Browser prefetch / favicon — ignore and keep listening.
                    oauth_flow::respond_404(request);
                }
                Err(e) => {
                    oauth_flow::respond_error(request);
                    return Err(e);
                }
            }
        };

        let tokens = oauth_flow::exchange_code(client_id, &code, &verifier, port)?;
        tokens::save_json(auth_dir, "oauth_tokens.json", &tokens)?;
        super::save_auth_meta(auth_dir, &crate::config::AuthMethod::Oauth)?;
        eprintln!("OAuth tokens saved.");
        Ok(())
    }

    fn check(&self, auth_dir: &Path) -> Result<()> {
        let tokens: OAuthTokens = tokens::load_json(auth_dir, "oauth_tokens.json")?;
        if tokens.expires_at < Utc::now() {
            self.refresh_tokens(auth_dir, &tokens)?;
        }
        Ok(())
    }

    fn authenticate(&self, auth_dir: &Path, builder: RequestBuilder) -> Result<RequestBuilder> {
        let mut tokens: OAuthTokens = tokens::load_json(auth_dir, "oauth_tokens.json")?;
        if tokens.expires_at < Utc::now() {
            tokens = self.refresh_tokens(auth_dir, &tokens)?;
        }
        Ok(builder.bearer_auth(&tokens.access_token))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_missing_tokens() {
        let cfg: PluginConfig = serde_json::from_str(
            r#"{"url":"https://x","project":"X","auth_method":"oauth","oauth_client_id":"c"}"#,
        )
        .unwrap();
        let auth = OAuthAuth::new(cfg);
        let dir = tempfile::tempdir().unwrap();
        assert!(auth.check(dir.path()).is_err());
    }

    #[test]
    fn client_id_missing() {
        let cfg: PluginConfig = serde_json::from_str(
            r#"{"url":"https://x","project":"X","auth_method":"oauth"}"#,
        )
        .unwrap();
        let auth = OAuthAuth::new(cfg);
        assert!(auth.client_id().is_err());
    }

    #[test]
    fn client_id_present() {
        let cfg: PluginConfig = serde_json::from_str(
            r#"{"url":"https://x","project":"X","auth_method":"oauth","oauth_client_id":"abc"}"#,
        )
        .unwrap();
        let auth = OAuthAuth::new(cfg);
        assert_eq!(auth.client_id().unwrap(), "abc");
    }
}
