use crate::auth::tokens::SessionCookie;
use crate::error::{PluginError, Result};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

fn form_selector() -> &'static scraper::Selector {
    static SEL: OnceLock<scraper::Selector> = OnceLock::new();
    SEL.get_or_init(|| scraper::Selector::parse("form").expect("static form selector"))
}

fn hidden_input_selector() -> &'static scraper::Selector {
    static SEL: OnceLock<scraper::Selector> = OnceLock::new();
    SEL.get_or_init(|| {
        scraper::Selector::parse("input[type=hidden]").expect("static input selector")
    })
}

/// Encode a JSON value using the native messaging wire protocol:
/// 4-byte little-endian length prefix + JSON bytes.
pub fn encode_native_message(value: &serde_json::Value) -> Result<Vec<u8>> {
    let json = serde_json::to_vec(value)?;
    let len = json.len() as u32;
    let mut buf = Vec::with_capacity(4 + json.len());
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&json);
    Ok(buf)
}

/// Decode a native messaging response: 4-byte LE length + JSON.
pub fn decode_native_message(data: &[u8]) -> Result<serde_json::Value> {
    if data.len() < 4 {
        return Err(PluginError::Auth("Helper response too short".into()));
    }
    let len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    if data.len() < 4 + len {
        return Err(PluginError::Auth(format!(
            "Helper response truncated: expected {} bytes, got {}",
            len,
            data.len() - 4
        )));
    }
    serde_json::from_slice(&data[4..4 + len])
        .map_err(|e| PluginError::Auth(format!("Helper response JSON: {}", e)))
}

/// Send a request to the helper binary and read the response.
pub fn call_helper(helper_path: &Path, request: &serde_json::Value) -> Result<serde_json::Value> {
    let mut child = Command::new(helper_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| PluginError::Auth(format!("Failed to spawn helper: {}", e)))?;

    let encoded = encode_native_message(request)?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(&encoded)?;
    }
    drop(child.stdin.take());

    let output = child.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PluginError::Auth(format!("Helper failed: {}", stderr)));
    }
    decode_native_message(&output.stdout)
}

/// Parse session cookies from the helper response.
/// Format: newline-delimited "key=value" pairs.
pub fn parse_session_cookies(raw: &str) -> Vec<SessionCookie> {
    raw.lines()
        .filter_map(|line| {
            let line = line.trim();
            let (name, value) = line.split_once('=')?;
            Some(SessionCookie {
                name: name.to_string(),
                value: value.to_string(),
            })
        })
        .collect()
}

/// Extract the IdP endpoint and SAML data from Jira's login page HTML.
pub fn parse_login_form(html: &str) -> Result<(String, String, Option<String>)> {
    parse_saml_form(html, "SAMLRequest")
}

/// Parse SAML response form from the IdP redirect page.
pub fn parse_saml_response_form(html: &str) -> Result<(String, String, Option<String>)> {
    parse_saml_form(html, "SAMLResponse")
}

fn parse_saml_form(html: &str, field_name: &str) -> Result<(String, String, Option<String>)> {
    let doc = scraper::Html::parse_document(html);
    // Pages can have multiple forms (search, language switcher, etc.).
    // Find the first form that contains a hidden input with the target name.
    for form in doc.select(form_selector()) {
        let mut saml_value = None;
        let mut relay_state = None;
        for input in form.select(hidden_input_selector()) {
            match input.value().attr("name") {
                Some(name) if name == field_name => {
                    saml_value = input.value().attr("value").map(String::from);
                }
                Some("RelayState") => {
                    relay_state = input.value().attr("value").map(String::from);
                }
                _ => {}
            }
        }
        if let Some(value) = saml_value {
            let action = form.value().attr("action").ok_or_else(|| {
                PluginError::Auth("SAML form has no action attribute".into())
            })?;
            return Ok((action.to_string(), value, relay_state));
        }
    }
    Err(PluginError::Auth(format!("No {} form found in HTML", field_name)))
}

/// Extract the host component from a URL.
pub fn extract_host(url_str: &str) -> Result<String> {
    url::Url::parse(url_str)
        .map_err(|e| PluginError::Auth(format!("bad URL {}: {}", url_str, e)))?
        .host_str()
        .map(|h| h.to_string())
        .ok_or_else(|| PluginError::Auth(format!("no host in URL: {}", url_str)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        let val = serde_json::json!({"hello": "world"});
        let encoded = encode_native_message(&val).unwrap();
        let decoded = decode_native_message(&encoded).unwrap();
        assert_eq!(decoded["hello"], "world");
    }

    #[test]
    fn decode_too_short() {
        assert!(decode_native_message(&[1, 2]).is_err());
    }

    #[test]
    fn decode_truncated() {
        let mut data = vec![10, 0, 0, 0];
        data.extend_from_slice(b"short");
        assert!(decode_native_message(&data).is_err());
    }

    #[test]
    fn parse_cookies() {
        let raw = "SSO_SESSION=abc\nJSESSIONID=xyz\n";
        let cookies = parse_session_cookies(raw);
        assert_eq!(cookies.len(), 2);
        assert_eq!(cookies[0].name, "SSO_SESSION");
        assert_eq!(cookies[1].name, "JSESSIONID");
    }

    #[test]
    fn parse_cookies_empty() {
        assert!(parse_session_cookies("").is_empty());
        assert!(parse_session_cookies("\n\n").is_empty());
    }

    #[test]
    fn parse_login_form_ok() {
        let html = r#"<html><body>
            <form action="https://idp.example.com/saml">
                <input type="hidden" name="SAMLRequest" value="req123"/>
                <input type="hidden" name="RelayState" value="relay"/>
            </form>
        </body></html>"#;
        let (action, saml, relay) = parse_login_form(html).unwrap();
        assert_eq!(action, "https://idp.example.com/saml");
        assert_eq!(saml, "req123");
        assert_eq!(relay, Some("relay".into()));
    }

    #[test]
    fn parse_login_form_no_relay() {
        let html = r#"<form action="https://idp.example.com/saml">
            <input type="hidden" name="SAMLRequest" value="req"/>
        </form>"#;
        let (_, _, relay) = parse_login_form(html).unwrap();
        assert!(relay.is_none());
    }

    #[test]
    fn parse_login_form_no_form() {
        assert!(parse_login_form("<html></html>").is_err());
    }

    #[test]
    fn parse_login_form_no_saml() {
        let html = r#"<form action="https://x"><input type="hidden" name="other" value="v"/></form>"#;
        assert!(parse_login_form(html).is_err());
    }

    #[test]
    fn parse_login_form_skips_non_saml_forms() {
        // Real pages have search forms, language selectors, etc. before
        // the SAML form — must skip them.
        let html = r#"<html><body>
            <form action="/search"><input type="text" name="q"/></form>
            <form action="/login"><input type="hidden" name="csrf" value="x"/></form>
            <form action="https://idp.example.com/saml">
                <input type="hidden" name="SAMLRequest" value="real"/>
            </form>
        </body></html>"#;
        let (action, saml, _) = parse_login_form(html).unwrap();
        assert_eq!(action, "https://idp.example.com/saml");
        assert_eq!(saml, "real");
    }

    #[test]
    fn extract_host_ok() {
        assert_eq!(extract_host("https://idp.example.com/path").unwrap(), "idp.example.com");
    }

    #[test]
    fn extract_host_bad_url() {
        assert!(extract_host("not-a-url").is_err());
    }

    #[test]
    fn parse_saml_response_form_ok() {
        let html = r#"<form action="https://jira.example.com/saml/acs">
            <input type="hidden" name="SAMLResponse" value="resp123"/>
            <input type="hidden" name="RelayState" value="rs"/>
        </form>"#;
        let (action, resp, relay) = parse_saml_response_form(html).unwrap();
        assert_eq!(action, "https://jira.example.com/saml/acs");
        assert_eq!(resp, "resp123");
        assert_eq!(relay, Some("rs".into()));
    }

    #[test]
    fn parse_saml_response_form_no_form() {
        assert!(parse_saml_response_form("<html></html>").is_err());
    }

    #[test]
    fn parse_saml_response_form_no_response() {
        let html = r#"<form action="https://x"><input type="hidden" name="other" value="v"/></form>"#;
        assert!(parse_saml_response_form(html).is_err());
    }
}
