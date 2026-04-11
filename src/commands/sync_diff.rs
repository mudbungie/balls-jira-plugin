use crate::config::PluginConfig;
use crate::jira::client::JiraClient;
use crate::jira::types::JiraIssue;
use crate::mapping;
use crate::types::{ExternalMeta, SyncCreate, SyncUpdate, Task};
use chrono::Utc;
use serde_json::Value;
use std::collections::BTreeMap;

pub fn issue_to_create(
    client: &JiraClient,
    config: &PluginConfig,
    issue: &JiraIssue,
) -> SyncCreate {
    let description = client.extract_description(issue);
    let priority = issue
        .fields
        .priority
        .as_ref()
        .map(|p| mapping::jira_priority_to_balls(&p.name))
        .unwrap_or(3);
    let status = mapping::jira_status_to_balls(&issue.fields.status.name, config);
    let task_type = mapping::jira_type_to_balls(&issue.fields.issuetype.name);

    SyncCreate {
        title: issue.fields.summary.clone(),
        task_type,
        priority,
        status,
        description,
        tags: issue.fields.labels.clone(),
        external: ExternalMeta {
            remote_key: issue.key.clone(),
            remote_url: client.browse_url(&issue.key),
            synced_at: Utc::now(),
        },
    }
}

pub fn diff_issue(
    client: &JiraClient,
    config: &PluginConfig,
    task: &Task,
    issue: &JiraIssue,
) -> Option<SyncUpdate> {
    let mut fields = BTreeMap::new();
    let mut notes = Vec::new();

    if issue.fields.summary != task.title {
        fields.insert("title".into(), Value::String(issue.fields.summary.clone()));
        notes.push(format!(
            "Title changed to \"{}\" in Jira",
            issue.fields.summary
        ));
    }

    let remote_desc = client.extract_description(issue);
    if remote_desc != task.description {
        fields.insert("description".into(), Value::String(remote_desc));
    }

    let remote_priority = issue
        .fields
        .priority
        .as_ref()
        .map(|p| mapping::jira_priority_to_balls(&p.name))
        .unwrap_or(3);
    if remote_priority != task.priority {
        fields.insert("priority".into(), Value::Number(remote_priority.into()));
    }

    let remote_status = mapping::jira_status_to_balls(&issue.fields.status.name, config);
    let task_status = task.status.to_lowercase().replace(' ', "_");
    if remote_status != task_status {
        fields.insert("status".into(), Value::String(remote_status.clone()));
        notes.push(format!(
            "Status changed to {} in Jira",
            issue.fields.status.name
        ));
    }

    if fields.is_empty() {
        return None;
    }

    let note = if notes.is_empty() {
        None
    } else {
        Some(notes.join("; "))
    };

    Some(SyncUpdate {
        task_id: task.id.clone(),
        fields,
        external: ExternalMeta {
            remote_key: issue.key.clone(),
            remote_url: client.browse_url(&issue.key),
            synced_at: Utc::now(),
        },
        add_note: note,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::pat::PatAuth;

    fn sample_config() -> PluginConfig {
        serde_json::from_str(r#"{"url":"https://x","project":"X"}"#).unwrap()
    }

    #[test]
    fn create_maps_fields() {
        let config = sample_config();
        let auth = PatAuth::new(config.clone());
        let dir = tempfile::tempdir().unwrap();
        let client = JiraClient::new(config.clone(), &auth, dir.path());

        let issue: JiraIssue = serde_json::from_str(
            r#"{
            "key": "X-1",
            "fields": {
                "summary": "Remote task",
                "issuetype": {"name": "Bug"},
                "priority": {"name": "High", "id": "2"},
                "status": {"name": "In Progress"},
                "description": null,
                "labels": ["imported"]
            }
        }"#,
        )
        .unwrap();

        let create = issue_to_create(&client, &config, &issue);
        assert_eq!(create.title, "Remote task");
        assert_eq!(create.task_type, "bug");
        assert_eq!(create.priority, 2);
        assert_eq!(create.status, "in_progress");
        assert_eq!(create.external.remote_key, "X-1");
        assert_eq!(create.tags, vec!["imported"]);
    }

    #[test]
    fn diff_no_changes() {
        let config = sample_config();
        let auth = PatAuth::new(config.clone());
        let dir = tempfile::tempdir().unwrap();
        let client = JiraClient::new(config.clone(), &auth, dir.path());

        let task = Task {
            id: "bl-1".into(),
            title: "Same".into(),
            task_type: "task".into(),
            priority: 3,
            status: "open".into(),
            description: "".into(),
            tags: vec![],
            external: BTreeMap::new(),
        };
        let issue: JiraIssue = serde_json::from_str(
            r#"{
            "key": "X-1",
            "fields": {
                "summary": "Same",
                "issuetype": {"name": "Task"},
                "priority": {"name": "Medium"},
                "status": {"name": "To Do"},
                "description": null
            }
        }"#,
        )
        .unwrap();

        assert!(diff_issue(&client, &config, &task, &issue).is_none());
    }

    #[test]
    fn diff_detects_changes() {
        let config = sample_config();
        let auth = PatAuth::new(config.clone());
        let dir = tempfile::tempdir().unwrap();
        let client = JiraClient::new(config.clone(), &auth, dir.path());

        let task = Task {
            id: "bl-1".into(),
            title: "Old".into(),
            task_type: "task".into(),
            priority: 3,
            status: "open".into(),
            description: "".into(),
            tags: vec![],
            external: BTreeMap::new(),
        };
        let issue: JiraIssue = serde_json::from_str(
            r#"{
            "key": "X-1",
            "fields": {
                "summary": "New Title",
                "issuetype": {"name": "Task"},
                "priority": {"name": "High"},
                "status": {"name": "In Progress"},
                "description": null
            }
        }"#,
        )
        .unwrap();

        let update = diff_issue(&client, &config, &task, &issue).unwrap();
        assert_eq!(update.task_id, "bl-1");
        assert!(update.fields.contains_key("title"));
        assert!(update.fields.contains_key("priority"));
        assert!(update.fields.contains_key("status"));
        assert!(update.add_note.is_some());
    }
}
