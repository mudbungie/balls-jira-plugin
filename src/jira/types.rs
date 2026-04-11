use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // fields used for deserialization completeness
pub struct JiraIssue {
    pub key: String,
    pub fields: JiraFields,
    #[serde(rename = "self")]
    pub self_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JiraFields {
    pub summary: String,
    pub issuetype: IssueType,
    pub priority: Option<JiraPriority>,
    pub status: JiraStatus,
    pub description: Option<Value>,
    #[serde(default)]
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IssueType {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct JiraPriority {
    pub name: String,
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JiraStatus {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SearchResults {
    pub issues: Vec<JiraIssue>,
    pub total: u64,
    #[serde(rename = "startAt")]
    pub start_at: u64,
    #[serde(rename = "maxResults")]
    pub max_results: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Transition {
    pub id: String,
    pub name: String,
    pub to: Option<TransitionTarget>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransitionTarget {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransitionsResponse {
    pub transitions: Vec<Transition>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct CreateIssueResponse {
    pub id: String,
    pub key: String,
    #[serde(rename = "self")]
    pub self_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateFields {
    pub project_key: String,
    pub summary: String,
    pub issuetype: String,
    pub description: Option<Value>,
    pub priority: Option<String>,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
}

impl UpdateFields {
    pub fn is_empty(&self) -> bool {
        self.summary.is_none()
            && self.description.is_none()
            && self.priority.is_none()
            && self.labels.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_jira_issue() {
        let json = r#"{
            "key": "PROJ-1",
            "self": "https://x/rest/api/3/issue/1",
            "fields": {
                "summary": "Test",
                "issuetype": {"name": "Task"},
                "priority": {"name": "Medium", "id": "3"},
                "status": {"name": "To Do"},
                "description": null,
                "labels": ["a"]
            }
        }"#;
        let issue: JiraIssue = serde_json::from_str(json).unwrap();
        assert_eq!(issue.key, "PROJ-1");
        assert_eq!(issue.fields.summary, "Test");
        assert_eq!(issue.fields.labels, vec!["a"]);
    }

    #[test]
    fn deserialize_search_results() {
        let json = r#"{
            "issues": [],
            "total": 0,
            "startAt": 0,
            "maxResults": 50
        }"#;
        let sr: SearchResults = serde_json::from_str(json).unwrap();
        assert_eq!(sr.total, 0);
    }

    #[test]
    fn deserialize_transitions() {
        let json = r#"{
            "transitions": [
                {"id": "1", "name": "Start", "to": {"name": "In Progress"}},
                {"id": "2", "name": "Done", "to": null}
            ]
        }"#;
        let tr: TransitionsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(tr.transitions.len(), 2);
        assert_eq!(tr.transitions[0].to.as_ref().unwrap().name, "In Progress");
        assert!(tr.transitions[1].to.is_none());
    }

    #[test]
    fn deserialize_create_response() {
        let json = r#"{"id":"10001","key":"PROJ-2","self":"https://x/rest/api/3/issue/10001"}"#;
        let cr: CreateIssueResponse = serde_json::from_str(json).unwrap();
        assert_eq!(cr.key, "PROJ-2");
    }

    #[test]
    fn update_fields_empty() {
        let uf = UpdateFields::default();
        assert!(uf.is_empty());
    }

    #[test]
    fn update_fields_not_empty() {
        let uf = UpdateFields {
            summary: Some("new".into()),
            ..Default::default()
        };
        assert!(!uf.is_empty());
    }

    #[test]
    fn deserialize_issue_no_priority() {
        let json = r#"{
            "key": "X-1",
            "fields": {
                "summary": "T",
                "issuetype": {"name": "Bug"},
                "priority": null,
                "status": {"name": "Open"},
                "description": null
            }
        }"#;
        let issue: JiraIssue = serde_json::from_str(json).unwrap();
        assert!(issue.fields.priority.is_none());
    }
}
