use serde_json::{json, Value};

/// Extract plain text description from Server API v2 response.
/// Server descriptions are plain text or wiki markup stored as a string.
/// Unexpected shapes return an empty string rather than leaking JSON.
pub fn extract_description(desc: &Value) -> String {
    match desc {
        Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

/// Build create issue payload for Server API v2.
pub fn build_create_payload(
    project_key: &str,
    summary: &str,
    issuetype: &str,
    description: Option<&str>,
    priority: Option<&str>,
    labels: &[String],
) -> Value {
    let mut fields = json!({
        "project": {"key": project_key},
        "summary": summary,
        "issuetype": {"name": issuetype},
    });
    if let Some(desc) = description {
        fields["description"] = json!(desc);
    }
    if let Some(p) = priority {
        fields["priority"] = json!({"name": p});
    }
    if !labels.is_empty() {
        fields["labels"] = json!(labels);
    }
    json!({"fields": fields})
}

/// Build update issue payload for Server API v2.
pub fn build_update_payload(
    summary: Option<&str>,
    description: Option<&str>,
    priority: Option<&str>,
    labels: Option<&[String]>,
) -> Value {
    let mut fields = serde_json::Map::new();
    if let Some(s) = summary {
        fields.insert("summary".into(), json!(s));
    }
    if let Some(d) = description {
        fields.insert("description".into(), json!(d));
    }
    if let Some(p) = priority {
        fields.insert("priority".into(), json!({"name": p}));
    }
    if let Some(l) = labels {
        fields.insert("labels".into(), json!(l));
    }
    json!({"fields": fields})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_description_string() {
        assert_eq!(
            extract_description(&json!("hello world")),
            "hello world"
        );
    }

    #[test]
    fn extract_description_null() {
        assert_eq!(extract_description(&json!(null)), "");
    }

    #[test]
    fn extract_description_other() {
        // Non-string JSON values (e.g. Cloud ADF leaking in) return empty
        // rather than a raw JSON string.
        let val = json!({"type": "doc"});
        assert_eq!(extract_description(&val), "");
    }

    #[test]
    fn build_create_minimal() {
        let payload = build_create_payload("PROJ", "title", "Task", None, None, &[]);
        assert_eq!(payload["fields"]["project"]["key"], "PROJ");
        assert_eq!(payload["fields"]["summary"], "title");
        assert_eq!(payload["fields"]["issuetype"]["name"], "Task");
    }

    #[test]
    fn build_create_full() {
        let labels = vec!["x".into()];
        let payload = build_create_payload(
            "PROJ", "title", "Bug", Some("desc"), Some("High"), &labels,
        );
        assert_eq!(payload["fields"]["description"], "desc");
        assert_eq!(payload["fields"]["priority"]["name"], "High");
        assert_eq!(payload["fields"]["labels"][0], "x");
    }

    #[test]
    fn build_update_empty() {
        let payload = build_update_payload(None, None, None, None);
        assert!(payload["fields"].as_object().unwrap().is_empty());
    }

    #[test]
    fn build_update_partial() {
        let payload = build_update_payload(Some("new"), None, Some("Low"), None);
        assert_eq!(payload["fields"]["summary"], "new");
        assert_eq!(payload["fields"]["priority"]["name"], "Low");
        assert!(payload["fields"].get("description").is_none());
    }

    #[test]
    fn build_update_labels() {
        let labels = vec!["a".into(), "b".into()];
        let payload = build_update_payload(None, None, None, Some(&labels));
        assert_eq!(payload["fields"]["labels"][0], "a");
    }
}
