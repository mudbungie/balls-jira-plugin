use std::io;

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("auth: {0}")]
    Auth(String),
    #[error("config: {0}")]
    Config(String),
    #[error("jira api {status}: {body}")]
    JiraApi { status: u16, body: String },
    #[error("mapping: {0}")]
    #[allow(dead_code)]
    Mapping(String),
    #[error("{0}")]
    #[allow(dead_code)]
    Other(String),
}

pub type Result<T> = std::result::Result<T, PluginError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_error_display() {
        let e: PluginError = io::Error::new(io::ErrorKind::NotFound, "gone").into();
        assert!(e.to_string().contains("gone"));
    }

    #[test]
    fn json_error_display() {
        let e: PluginError = serde_json::from_str::<String>("bad").unwrap_err().into();
        assert!(e.to_string().contains("json"));
    }

    #[test]
    fn auth_error_display() {
        let e = PluginError::Auth("expired".into());
        assert_eq!(e.to_string(), "auth: expired");
    }

    #[test]
    fn config_error_display() {
        let e = PluginError::Config("missing url".into());
        assert_eq!(e.to_string(), "config: missing url");
    }

    #[test]
    fn jira_api_error_display() {
        let e = PluginError::JiraApi {
            status: 404,
            body: "not found".into(),
        };
        assert_eq!(e.to_string(), "jira api 404: not found");
    }

    #[test]
    fn mapping_error_display() {
        let e = PluginError::Mapping("unknown status".into());
        assert_eq!(e.to_string(), "mapping: unknown status");
    }

    #[test]
    fn other_error_display() {
        let e = PluginError::Other("oops".into());
        assert_eq!(e.to_string(), "oops");
    }
}
