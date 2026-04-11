use super::saml;
use super::tokens::{self, DeviceSession, SessionCookie};
use super::AuthProvider;
use crate::config::PluginConfig;
use crate::error::{PluginError, Result};
use chrono::Utc;
use reqwest::blocking::RequestBuilder;
use std::path::{Path, PathBuf};

pub struct DeviceAuth {
    config: PluginConfig,
}

impl DeviceAuth {
    pub fn new(config: PluginConfig) -> Self {
        Self { config }
    }

    fn helper_path(&self) -> Result<PathBuf> {
        match &self.config.device_auth_helper_path {
            Some(p) => {
                let path = PathBuf::from(p);
                if path.exists() {
                    Ok(path)
                } else {
                    Err(PluginError::Auth(format!(
                        "device auth helper not found at configured path: {}",
                        p
                    )))
                }
            }
            None => Err(PluginError::Auth(
                "device_auth_helper_path must be set in plugin config".into(),
            )),
        }
    }
}

impl AuthProvider for DeviceAuth {
    fn setup(&self, auth_dir: &Path) -> Result<()> {
        let helper = self.helper_path()?;
        let login_url = format!(
            "{}/login.action?os_destination=/rest/api/2/myself",
            self.config.url.trim_end_matches('/')
        );

        // Step 1: Discover IdP
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .cookie_store(true)
            .build()?;
        let resp = client.get(&login_url).send()?;
        let html = resp.text()?;
        let (action, saml_request, relay_state) = saml::parse_login_form(&html)?;
        let idp_host = saml::extract_host(&action)?;

        // Step 2: Get device-signed credentials from local helper binary
        let now = Utc::now().format("%a, %d %b %Y %H:%M:%S %z").to_string();
        let signing_string = format!("date: {}\nhost: {}", now, idp_host);
        let request = serde_json::json!({
            "operation": "getAuthData",
            "key": "signingString",
            "value": signing_string,
            "browserClient": "firefox"
        });
        let response = saml::call_helper(&helper, &request)?;
        let device_token = response["deviceToken"]
            .as_str()
            .ok_or_else(|| PluginError::Auth("no deviceToken in helper response".into()))?;
        let signature = response["signature"]
            .as_str()
            .ok_or_else(|| PluginError::Auth("no signature in helper response".into()))?;
        let seed_cookies_raw = response["sessionCookie"].as_str().unwrap_or("");
        let seed_cookies = saml::parse_session_cookies(seed_cookies_raw);

        // Step 3: POST to IdP with signed credentials
        let auth_header = format!(
            "Signature keyId=\"{}\",version=\"1\",algorithm=\"rsa-sha256\",\
             headers=\"date host\",signature=\"{}\"",
            device_token, signature
        );
        let mut form = vec![("SAMLRequest", saml_request)];
        if let Some(rs) = &relay_state {
            form.push(("RelayState", rs.clone()));
        }
        let mut idp_req = client
            .post(&action)
            .header("Authorization", &auth_header)
            .header("Date", &now)
            .header("Host", &idp_host)
            .form(&form);
        for c in &seed_cookies {
            idp_req = idp_req.header("Cookie", format!("{}={}", c.name, c.value));
        }
        let idp_resp = idp_req.send()?;
        let redirect_url = idp_resp
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| PluginError::Auth("no redirect from IdP".into()))?
            .to_string();

        // Step 4: Follow redirect
        let redirect_resp = client.get(&redirect_url).send()?;
        let redirect_html = redirect_resp.text()?;
        let (consumer_url, saml_response, relay_state2) =
            saml::parse_saml_response_form(&redirect_html)?;

        // Step 5: Complete SAML handshake
        let mut final_form = vec![("SAMLResponse", saml_response)];
        if let Some(rs) = &relay_state2 {
            final_form.push(("RelayState", rs.clone()));
        }
        let final_client = reqwest::blocking::Client::builder()
            .cookie_store(true)
            .build()?;
        let final_resp = final_client.post(&consumer_url).form(&final_form).send()?;

        let mut session_cookies = Vec::new();
        for cookie in final_resp.cookies() {
            session_cookies.push(SessionCookie {
                name: cookie.name().to_string(),
                value: cookie.value().to_string(),
            });
        }
        if session_cookies.is_empty() {
            return Err(PluginError::Auth("no session cookies received".into()));
        }

        let session = DeviceSession {
            cookies: session_cookies,
            obtained_at: Utc::now(),
        };
        tokens::save_json(auth_dir, "session.json", &session)?;
        super::save_auth_meta(auth_dir, &crate::config::AuthMethod::DeviceAuth)?;
        eprintln!("Device auth session established.");
        Ok(())
    }

    fn check(&self, auth_dir: &Path) -> Result<()> {
        let _session: DeviceSession = tokens::load_json(auth_dir, "session.json")?;
        Ok(())
    }

    fn authenticate(&self, auth_dir: &Path, builder: RequestBuilder) -> Result<RequestBuilder> {
        let session: DeviceSession = tokens::load_json(auth_dir, "session.json")?;
        let cookie_str: String = session
            .cookies
            .iter()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; ");
        Ok(builder.header("Cookie", cookie_str))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device_auth_config() -> &'static str {
        r#"{"url":"https://x","project":"X","auth_method":"device_auth"}"#
    }

    #[test]
    fn check_missing_session() {
        let cfg: PluginConfig = serde_json::from_str(device_auth_config()).unwrap();
        let auth = DeviceAuth::new(cfg);
        let dir = tempfile::tempdir().unwrap();
        assert!(auth.check(dir.path()).is_err());
    }

    #[test]
    fn authenticate_adds_cookies() {
        let cfg: PluginConfig = serde_json::from_str(device_auth_config()).unwrap();
        let auth = DeviceAuth::new(cfg);
        let dir = tempfile::tempdir().unwrap();
        let session = DeviceSession {
            cookies: vec![
                SessionCookie { name: "A".into(), value: "1".into() },
                SessionCookie { name: "B".into(), value: "2".into() },
            ],
            obtained_at: Utc::now(),
        };
        tokens::save_json(dir.path(), "session.json", &session).unwrap();
        let client = reqwest::blocking::Client::new();
        let builder = client.get("https://example.com");
        let builder = auth.authenticate(dir.path(), builder).unwrap();
        let req = builder.build().unwrap();
        let cookie = req.headers().get("Cookie").unwrap().to_str().unwrap();
        assert!(cookie.contains("A=1"));
        assert!(cookie.contains("B=2"));
    }

    #[test]
    fn helper_path_not_configured() {
        let cfg: PluginConfig = serde_json::from_str(device_auth_config()).unwrap();
        let auth = DeviceAuth::new(cfg);
        let err = auth.helper_path().unwrap_err();
        assert!(err.to_string().contains("device_auth_helper_path"));
    }

    #[test]
    fn helper_path_configured_missing() {
        let cfg: PluginConfig = serde_json::from_str(
            r#"{"url":"https://x","project":"X","auth_method":"device_auth","device_auth_helper_path":"/nonexistent"}"#,
        )
        .unwrap();
        let auth = DeviceAuth::new(cfg);
        let err = auth.helper_path().unwrap_err();
        assert!(err.to_string().contains("/nonexistent"));
    }
}
