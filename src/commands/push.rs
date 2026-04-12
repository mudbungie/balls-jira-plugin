use crate::auth;
use crate::config::PluginConfig;
use crate::error::{PluginError, Result};
use crate::jira::client::JiraClient;
use crate::jira::discover::DiscoveredMappings;
use crate::jira::types::{CreateFields, UpdateFields};
use crate::mapping;
use crate::types::{PushResponse, Task};
use chrono::Utc;
use serde_json::Value;
use std::io::Read;
use std::path::Path;

pub fn run(task_id: &str, config_path: &Path, auth_dir: &Path) -> Result<()> {
    let config = PluginConfig::load(config_path)?;
    let provider = auth::create_provider(&config);
    let mut client = JiraClient::new(config.clone(), provider.as_ref(), auth_dir);
    let discovered = client.discover()?;

    let mut stdin_buf = String::new();
    std::io::stdin().read_to_string(&mut stdin_buf)?;
    let task: Task = serde_json::from_str(&stdin_buf)?;
    if task.id != task_id {
        return Err(PluginError::Other(format!(
            "--task {} does not match stdin task id {}",
            task_id, task.id
        )));
    }

    let response = push_task(&client, &config, &discovered, &task)?;
    if let Some(resp) = response {
        println!("{}", serde_json::to_string(&resp)?);
    }
    Ok(())
}

pub fn push_task(
    client: &JiraClient,
    config: &PluginConfig,
    discovered: &DiscoveredMappings,
    task: &Task,
) -> Result<Option<PushResponse>> {
    match task.remote_key() {
        Some(key) => update_remote(client, config, discovered, task, key),
        None if config.create_in_remote => create_remote(client, config, discovered, task),
        None => Ok(None),
    }
}

fn create_remote(
    client: &JiraClient,
    config: &PluginConfig,
    discovered: &DiscoveredMappings,
    task: &Task,
) -> Result<Option<PushResponse>> {
    let fields = CreateFields {
        project_key: config.project.clone(),
        summary: task.title.clone(),
        issuetype: mapping::balls_type_to_jira(&task.task_type),
        description: if task.description.is_empty() {
            None
        } else {
            Some(Value::String(task.description.clone()))
        },
        priority: Some(mapping::balls_priority_to_jira(task.priority, Some(discovered))),
        labels: task.tags.clone(),
    };
    let (key, browse_url) = client.create_issue(&fields)?;

    // Always attempt a transition to the task's status; transition_issue is
    // a no-op when the issue already matches. This handles Jira workflows
    // whose default landing state differs from the balls "open" state.
    client.transition_issue(&key, &task.status, config, Some(discovered))?;

    Ok(Some(PushResponse {
        remote_key: key,
        remote_url: browse_url,
        synced_at: Utc::now(),
    }))
}

fn update_remote(
    client: &JiraClient,
    config: &PluginConfig,
    discovered: &DiscoveredMappings,
    task: &Task,
    key: &str,
) -> Result<Option<PushResponse>> {
    let remote = client.get_issue(key)?;
    let mut update = UpdateFields::default();

    if remote.fields.summary != task.title {
        update.summary = Some(task.title.clone());
    }
    let remote_desc = client.extract_description(&remote);
    if remote_desc != task.description {
        update.description = Some(Value::String(task.description.clone()));
    }
    let remote_priority = remote
        .fields
        .priority
        .as_ref()
        .map(|p| mapping::jira_priority_to_balls(&p.name, Some(discovered)))
        .unwrap_or(3);
    if remote_priority != task.priority {
        update.priority = Some(mapping::balls_priority_to_jira(
            task.priority,
            Some(discovered),
        ));
    }
    if remote.fields.labels != task.tags {
        update.labels = Some(task.tags.clone());
    }
    client.update_issue(key, &update)?;

    let remote_balls = mapping::jira_status_to_balls(
        &remote.fields.status.name,
        config,
        Some(discovered),
    );
    let task_status = task.status.to_lowercase().replace(' ', "_");
    if remote_balls != task_status
        && (task_status != "closed" || config.close_in_remote)
    {
        client.transition_issue(key, &task.status, config, Some(discovered))?;
    }

    Ok(Some(PushResponse {
        remote_key: key.to_string(),
        remote_url: client.browse_url(key),
        synced_at: Utc::now(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_task(remote_key: Option<&str>) -> Task {
        let external = match remote_key {
            Some(k) => serde_json::json!({"jira": {"remote_key": k}}),
            None => serde_json::json!({}),
        };
        Task {
            id: "bl-test".into(),
            title: "Test task".into(),
            task_type: "Task".into(),
            priority: 2,
            status: "Open".into(),
            description: "A test".into(),
            tags: vec!["test".into()],
            external: external.as_object().unwrap().clone().into_iter().collect(),
        }
    }

    #[test]
    fn task_without_remote_key_and_no_create() {
        let task = sample_task(None);
        let config: PluginConfig = serde_json::from_str(
            r#"{"url":"https://x","project":"X","create_in_remote":false}"#,
        )
        .unwrap();
        assert!(!config.create_in_remote);
        assert!(task.remote_key().is_none());
    }

    #[test]
    fn task_with_remote_key() {
        let task = sample_task(Some("PROJ-1"));
        assert_eq!(task.remote_key(), Some("PROJ-1"));
    }
}
