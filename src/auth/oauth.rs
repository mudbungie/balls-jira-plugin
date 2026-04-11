use super::tokens::{self, OAuthTokens};
use super::AuthProvider;
use crate::config::PluginConfig;
use crate::error::{PluginError, Result};
use chrono::{Duration, Utc};
use reqwest::blocking::RequestBuilder;
use sha2::{Digest, Sha256};
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
            let body = resp.text().unwrap_or_default();
            return Err(PluginError::Auth(format!("token refresh failed: {}", body)));
        }
        let body: serde_json::Value = resp.json()?;
        let new_tokens = OAuthTokens {
            access_token: body["access_token"]
                .as_str()
                .ok_or_else(|| PluginError::Auth("no access_token in response".into()))?
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

pub fn generate_pkce() -> (String, String) {
    use rand::Rng;
    let verifier: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let digest = hasher.finalize();
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
    (verifier, challenge)
}

pub fn build_authorize_url(client_id: &str, callback_port: u16, challenge: &str) -> String {
    format!(
        "https://auth.atlassian.com/authorize?\
         audience=api.atlassian.com&\
         client_id={}&\
         scope=read%3Ajira-work%20write%3Ajira-work%20offline_access&\
         redirect_uri=http%3A%2F%2F127.0.0.1%3A{}%2Fcallback&\
         state=balls-plugin&\
         response_type=code&\
         prompt=consent&\
         code_challenge={}&\
         code_challenge_method=S256",
        client_id, callback_port, challenge
    )
}

use base64::Engine;

impl AuthProvider for OAuthAuth {
    fn setup(&self, auth_dir: &Path) -> Result<()> {
        let client_id = self.client_id()?;
        let port = self.config.oauth_callback_port.unwrap_or(19472);
        let (verifier, challenge) = generate_pkce();
        let url = build_authorize_url(client_id, port, &challenge);

        eprintln!("Opening browser for Atlassian OAuth...");
        eprintln!("If the browser doesn't open, visit:\n{}", url);
        let _ = open::that(&url);

        let server = tiny_http::Server::http(format!("127.0.0.1:{}", port))
            .map_err(|e| PluginError::Auth(format!("callback server: {}", e)))?;
        eprintln!("Waiting for OAuth callback on port {}...", port);

        let request = server
            .recv()
            .map_err(|e| PluginError::Auth(format!("callback recv: {}", e)))?;
        let url_str = request.url().to_string();
        let code = url_str
            .split("code=")
            .nth(1)
            .and_then(|s| s.split('&').next())
            .ok_or_else(|| PluginError::Auth("no code in callback".into()))?
            .to_string();

        let html = "<html><body><h2>Authentication successful!</h2>\
                     <p>You can close this tab.</p></body></html>";
        let response = tiny_http::Response::from_string(html)
            .with_header("Content-Type: text/html".parse::<tiny_http::Header>().unwrap());
        let _ = request.respond(response);

        let client = reqwest::blocking::Client::new();
        let resp = client
            .post("https://auth.atlassian.com/oauth/token")
            .json(&serde_json::json!({
                "grant_type": "authorization_code",
                "client_id": client_id,
                "code": code,
                "redirect_uri": format!("http://127.0.0.1:{}/callback", port),
                "code_verifier": verifier,
            }))
            .send()?;
        if !resp.status().is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(PluginError::Auth(format!("token exchange failed: {}", body)));
        }
        let body: serde_json::Value = resp.json()?;
        let access_token = body["access_token"]
            .as_str()
            .ok_or_else(|| PluginError::Auth("no access_token".into()))?;

        // Fetch cloud ID for the configured site
        let sites_resp = client
            .get("https://api.atlassian.com/oauth/token/accessible-resources")
            .bearer_auth(access_token)
            .send()?;
        let sites: Vec<serde_json::Value> = sites_resp.json()?;
        let cloud_id = sites
            .first()
            .and_then(|s| s["id"].as_str())
            .ok_or_else(|| PluginError::Auth("no accessible cloud sites".into()))?
            .to_string();

        let tokens = OAuthTokens {
            access_token: access_token.to_string(),
            refresh_token: body["refresh_token"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            expires_at: Utc::now()
                + Duration::seconds(body["expires_in"].as_i64().unwrap_or(3600)),
            cloud_id,
        };
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
    fn pkce_verifier_length() {
        let (verifier, challenge) = generate_pkce();
        assert_eq!(verifier.len(), 64);
        assert!(!challenge.is_empty());
        assert!(!challenge.contains('='));
    }

    #[test]
    fn authorize_url_contains_params() {
        let url = build_authorize_url("my-client", 19472, "ch4ll3ng3");
        assert!(url.contains("client_id=my-client"));
        assert!(url.contains("19472"));
        assert!(url.contains("ch4ll3ng3"));
        assert!(url.contains("S256"));
    }

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
