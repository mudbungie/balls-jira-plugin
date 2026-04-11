use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Task as received on stdin from balls core.
/// We only deserialize the fields we need; extras are ignored.
#[derive(Debug, Clone, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub task_type: String,
    pub priority: u8,
    pub status: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub external: BTreeMap<String, Value>,
}

impl Task {
    pub fn jira_external(&self) -> Option<&Value> {
        self.external.get("jira")
    }

    pub fn remote_key(&self) -> Option<&str> {
        self.jira_external()
            .and_then(|v| v.get("remote_key"))
            .and_then(|v| v.as_str())
    }
}

/// What we emit on stdout after a push.
#[derive(Debug, Serialize)]
pub struct PushResponse {
    pub remote_key: String,
    pub remote_url: String,
    pub synced_at: DateTime<Utc>,
}

/// Full sync report emitted on stdout.
#[derive(Debug, Default, Serialize)]
pub struct SyncReport {
    pub created: Vec<SyncCreate>,
    pub updated: Vec<SyncUpdate>,
    pub deleted: Vec<SyncDelete>,
}

#[derive(Debug, Serialize)]
pub struct SyncCreate {
    pub title: String,
    #[serde(rename = "type")]
    pub task_type: String,
    pub priority: u8,
    pub status: String,
    pub description: String,
    pub tags: Vec<String>,
    pub external: ExternalMeta,
}

#[derive(Debug, Serialize)]
pub struct SyncUpdate {
    pub task_id: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Value>,
    pub external: ExternalMeta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add_note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SyncDelete {
    pub task_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExternalMeta {
    pub remote_key: String,
    pub remote_url: String,
    pub synced_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_task() -> Task {
        serde_json::from_str(r#"{
            "id": "bl-a1b2",
            "title": "Fix bug",
            "task_type": "Bug",
            "priority": 2,
            "status": "Open",
            "description": "Something broke",
            "tags": ["urgent"],
            "external": {
                "jira": {"remote_key": "PROJ-123", "remote_url": "https://x/PROJ-123"}
            }
        }"#).unwrap()
    }

    #[test]
    fn deserialize_task() {
        let t = sample_task();
        assert_eq!(t.id, "bl-a1b2");
        assert_eq!(t.priority, 2);
        assert_eq!(t.tags, vec!["urgent"]);
    }

    #[test]
    fn remote_key_present() {
        let t = sample_task();
        assert_eq!(t.remote_key(), Some("PROJ-123"));
    }

    #[test]
    fn remote_key_absent() {
        let t: Task = serde_json::from_str(r#"{
            "id": "bl-x", "title": "t", "task_type": "Task",
            "priority": 3, "status": "Open", "description": ""
        }"#).unwrap();
        assert_eq!(t.remote_key(), None);
    }

    #[test]
    fn jira_external_present() {
        let t = sample_task();
        assert!(t.jira_external().is_some());
    }

    #[test]
    fn jira_external_absent() {
        let t: Task = serde_json::from_str(r#"{
            "id": "bl-x", "title": "t", "task_type": "Task",
            "priority": 3, "status": "Open", "description": ""
        }"#).unwrap();
        assert!(t.jira_external().is_none());
    }

    #[test]
    fn serialize_push_response() {
        let r = PushResponse {
            remote_key: "PROJ-1".into(),
            remote_url: "https://x/PROJ-1".into(),
            synced_at: Utc::now(),
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("PROJ-1"));
    }

    #[test]
    fn serialize_sync_report_empty() {
        let r = SyncReport::default();
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"created\":[]"));
    }

    #[test]
    fn serialize_sync_delete() {
        let d = SyncDelete {
            task_id: "bl-x".into(),
            reason: "gone".into(),
        };
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("gone"));
    }

    #[test]
    fn serialize_sync_update_skips_empty() {
        let u = SyncUpdate {
            task_id: "bl-x".into(),
            fields: BTreeMap::new(),
            external: ExternalMeta {
                remote_key: "K-1".into(),
                remote_url: "https://x".into(),
                synced_at: Utc::now(),
            },
            add_note: None,
        };
        let json = serde_json::to_string(&u).unwrap();
        assert!(!json.contains("fields"));
        assert!(!json.contains("add_note"));
    }
}
