//! OAuth 2.0 helpers: PKCE, state, callback parsing, token exchange,
//! and the local HTTP listener responses. Extracted from `oauth.rs` to
//! keep files under the source-size limit and isolate the crypto bits.

use super::tokens::OAuthTokens;
use crate::error::{PluginError, Result};
use base64::Engine;
use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};

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

pub fn generate_state() -> String {
    use rand::Rng;
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

pub fn build_authorize_url(
    client_id: &str,
    callback_port: u16,
    challenge: &str,
    state: &str,
) -> String {
    format!(
        "https://auth.atlassian.com/authorize?\
         audience=api.atlassian.com&\
         client_id={}&\
         scope=read%3Ajira-work%20write%3Ajira-work%20offline_access&\
         redirect_uri=http%3A%2F%2F127.0.0.1%3A{}%2Fcallback&\
         state={}&\
         response_type=code&\
         prompt=consent&\
         code_challenge={}&\
         code_challenge_method=S256",
        client_id, callback_port, state, challenge
    )
}

/// Parse an OAuth callback request URL (path + query) and extract the
/// code after validating the state against expected_state.
/// Rejects requests for paths other than /callback.
pub fn parse_callback(request_url: &str, expected_state: &str) -> Result<String> {
    let parsed = url::Url::parse(&format!("http://127.0.0.1{}", request_url))
        .map_err(|e| PluginError::Auth(format!("Invalid callback URL: {}", e)))?;
    if parsed.path() != "/callback" {
        return Err(PluginError::Auth(format!(
            "Unexpected request path: {}",
            parsed.path()
        )));
    }
    let mut code = None;
    let mut state = None;
    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "code" => code = Some(v.into_owned()),
            "state" => state = Some(v.into_owned()),
            "error" => {
                return Err(PluginError::Auth(format!(
                    "OAuth authorization error: {}",
                    v
                )))
            }
            _ => {}
        }
    }
    let code = code.ok_or_else(|| PluginError::Auth("No code in callback".into()))?;
    let state = state.ok_or_else(|| PluginError::Auth("No state in callback".into()))?;
    if state != expected_state {
        return Err(PluginError::Auth(
            "OAuth state mismatch (possible CSRF)".into(),
        ));
    }
    Ok(code)
}

pub fn respond_ok(request: tiny_http::Request) {
    let html = "<html><body><h2>Authentication successful</h2>\
                 <p>You can close this tab.</p></body></html>";
    let response = tiny_http::Response::from_string(html).with_header(
        "Content-Type: text/html"
            .parse::<tiny_http::Header>()
            .expect("static header"),
    );
    let _ = request.respond(response);
}

pub fn respond_404(request: tiny_http::Request) {
    let _ = request.respond(tiny_http::Response::empty(404));
}

pub fn respond_error(request: tiny_http::Request) {
    let html = "<html><body><h2>Authentication failed</h2>\
                 <p>See terminal for details.</p></body></html>";
    let response = tiny_http::Response::from_string(html)
        .with_status_code(400)
        .with_header(
            "Content-Type: text/html"
                .parse::<tiny_http::Header>()
                .expect("static header"),
        );
    let _ = request.respond(response);
}

pub fn exchange_code(
    client_id: &str,
    code: &str,
    verifier: &str,
    port: u16,
) -> Result<OAuthTokens> {
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
        return Err(PluginError::Auth(format!(
            "Token exchange failed: HTTP {}",
            resp.status()
        )));
    }
    let body: serde_json::Value = resp.json()?;
    let access_token = body["access_token"]
        .as_str()
        .ok_or_else(|| PluginError::Auth("No access_token in exchange response".into()))?;

    let sites_resp = client
        .get("https://api.atlassian.com/oauth/token/accessible-resources")
        .bearer_auth(access_token)
        .send()?;
    let sites: Vec<serde_json::Value> = sites_resp.json()?;
    let cloud_id = sites
        .first()
        .and_then(|s| s["id"].as_str())
        .ok_or_else(|| PluginError::Auth("No accessible cloud sites".into()))?
        .to_string();

    Ok(OAuthTokens {
        access_token: access_token.to_string(),
        refresh_token: body["refresh_token"].as_str().unwrap_or("").to_string(),
        expires_at: Utc::now()
            + Duration::seconds(body["expires_in"].as_i64().unwrap_or(3600)),
        cloud_id,
    })
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
    fn generate_state_unique_and_long() {
        let a = generate_state();
        let b = generate_state();
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
    }

    #[test]
    fn authorize_url_contains_params() {
        let url = build_authorize_url("my-client", 19472, "ch4ll3ng3", "st8t");
        assert!(url.contains("client_id=my-client"));
        assert!(url.contains("19472"));
        assert!(url.contains("ch4ll3ng3"));
        assert!(url.contains("state=st8t"));
        assert!(url.contains("S256"));
    }

    #[test]
    fn parse_callback_ok() {
        let code = parse_callback("/callback?code=abc123&state=expected", "expected").unwrap();
        assert_eq!(code, "abc123");
    }

    #[test]
    fn parse_callback_wrong_path() {
        assert!(parse_callback("/favicon.ico", "expected").is_err());
    }

    #[test]
    fn parse_callback_missing_code() {
        assert!(parse_callback("/callback?state=expected", "expected").is_err());
    }

    #[test]
    fn parse_callback_missing_state() {
        assert!(parse_callback("/callback?code=abc", "expected").is_err());
    }

    #[test]
    fn parse_callback_state_mismatch() {
        let err = parse_callback("/callback?code=abc&state=wrong", "expected").unwrap_err();
        assert!(err.to_string().contains("CSRF"));
    }

    #[test]
    fn parse_callback_oauth_error() {
        let err =
            parse_callback("/callback?error=access_denied&state=expected", "expected")
                .unwrap_err();
        assert!(err.to_string().contains("access_denied"));
    }

    #[test]
    fn parse_callback_url_encoded() {
        let code = parse_callback(
            "/callback?code=abc%2Fdef&state=expected",
            "expected",
        )
        .unwrap();
        assert_eq!(code, "abc/def");
    }
}
