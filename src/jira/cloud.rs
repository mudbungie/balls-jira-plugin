use serde_json::{json, Value};

/// Convert plain text to Atlassian Document Format (ADF) for Cloud API v3.
pub fn text_to_adf(text: &str) -> Value {
    if text.is_empty() {
        return json!({
            "version": 1,
            "type": "doc",
            "content": []
        });
    }
    let paragraphs: Vec<Value> = text
        .split('\n')
        .map(|line| {
            if line.is_empty() {
                json!({"type": "paragraph", "content": []})
            } else {
                json!({
                    "type": "paragraph",
                    "content": [{"type": "text", "text": line}]
                })
            }
        })
        .collect();
    json!({
        "version": 1,
        "type": "doc",
        "content": paragraphs
    })
}

/// Extract plain text from ADF.
pub fn adf_to_text(adf: &Value) -> String {
    let mut lines = Vec::new();
    if let Some(content) = adf.get("content").and_then(|c| c.as_array()) {
        for node in content {
            lines.push(extract_text_from_node(node));
        }
    }
    lines.join("\n")
}

fn extract_text_from_node(node: &Value) -> String {
    if let Some(text) = node.get("text").and_then(|t| t.as_str()) {
        return text.to_string();
    }
    if let Some(content) = node.get("content").and_then(|c| c.as_array()) {
        return content.iter().map(extract_text_from_node).collect::<Vec<_>>().join("");
    }
    String::new()
}

/// Build create issue payload for Cloud API v3.
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
        fields["description"] = text_to_adf(desc);
    }
    if let Some(p) = priority {
        fields["priority"] = json!({"name": p});
    }
    if !labels.is_empty() {
        fields["labels"] = json!(labels);
    }
    json!({"fields": fields})
}

/// Build update issue payload for Cloud API v3.
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
        fields.insert("description".into(), text_to_adf(d));
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
    fn text_to_adf_empty() {
        let adf = text_to_adf("");
        assert_eq!(adf["content"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn text_to_adf_single_line() {
        let adf = text_to_adf("hello world");
        let content = adf["content"].as_array().unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["content"][0]["text"], "hello world");
    }

    #[test]
    fn text_to_adf_multiline() {
        let adf = text_to_adf("line1\nline2\n\nline4");
        let content = adf["content"].as_array().unwrap();
        assert_eq!(content.len(), 4);
        assert_eq!(content[2]["content"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn adf_to_text_simple() {
        let adf = text_to_adf("hello\nworld");
        let text = adf_to_text(&adf);
        assert_eq!(text, "hello\nworld");
    }

    #[test]
    fn adf_to_text_empty() {
        let adf = json!({"version": 1, "type": "doc", "content": []});
        assert_eq!(adf_to_text(&adf), "");
    }

    #[test]
    fn adf_to_text_null() {
        assert_eq!(adf_to_text(&json!(null)), "");
    }

    #[test]
    fn build_create_minimal() {
        let payload = build_create_payload("PROJ", "title", "Task", None, None, &[]);
        assert_eq!(payload["fields"]["project"]["key"], "PROJ");
        assert_eq!(payload["fields"]["summary"], "title");
    }

    #[test]
    fn build_create_full() {
        let labels = vec!["a".into(), "b".into()];
        let payload =
            build_create_payload("PROJ", "title", "Bug", Some("desc"), Some("High"), &labels);
        assert!(payload["fields"].get("description").is_some());
        assert_eq!(payload["fields"]["priority"]["name"], "High");
    }

    #[test]
    fn build_update_empty() {
        let payload = build_update_payload(None, None, None, None);
        assert!(payload["fields"].as_object().unwrap().is_empty());
    }

    #[test]
    fn build_update_partial() {
        let payload = build_update_payload(Some("new title"), None, None, None);
        assert_eq!(payload["fields"]["summary"], "new title");
        assert!(payload["fields"].get("description").is_none());
    }
}
