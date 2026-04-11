use super::sync_diff;
use crate::auth;
use crate::config::PluginConfig;
use crate::error::Result;
use crate::jira::client::JiraClient;
use crate::types::{SyncDelete, SyncReport, Task};
use std::collections::{BTreeMap, HashSet};
use std::io::Read;
use std::path::Path;

pub fn run(task_filter: Option<&str>, config_path: &Path, auth_dir: &Path) -> Result<()> {
    let config = PluginConfig::load(config_path)?;
    let provider = auth::create_provider(&config);
    let client = JiraClient::new(config.clone(), provider.as_ref(), auth_dir);

    let mut stdin_buf = String::new();
    std::io::stdin().read_to_string(&mut stdin_buf)?;
    let tasks: Vec<Task> = serde_json::from_str(&stdin_buf)?;

    let report = build_sync_report(&client, &config, &tasks, task_filter)?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}

pub fn build_sync_report(
    client: &JiraClient,
    config: &PluginConfig,
    tasks: &[Task],
    task_filter: Option<&str>,
) -> Result<SyncReport> {
    let mut local_by_key: BTreeMap<String, &Task> = BTreeMap::new();
    for task in tasks {
        if let Some(key) = task.remote_key() {
            local_by_key.insert(key.to_string(), task);
        }
    }

    if let Some(filter) = task_filter {
        return sync_single(client, config, tasks, &local_by_key, filter);
    }

    let jql = config.effective_sync_filter();
    let remote_issues = client.search(&jql)?;
    let mut report = SyncReport::default();
    let mut seen_keys = HashSet::new();

    for issue in &remote_issues {
        seen_keys.insert(issue.key.clone());
        match local_by_key.get(&issue.key) {
            None => report.created.push(sync_diff::issue_to_create(client, config, issue)),
            Some(task) => {
                if let Some(update) = sync_diff::diff_issue(client, config, task, issue) {
                    report.updated.push(update);
                }
            }
        }
    }

    for (key, task) in &local_by_key {
        if !seen_keys.contains(key) {
            report.deleted.push(SyncDelete {
                task_id: task.id.clone(),
                reason: format!("Issue {} no longer matches sync filter", key),
            });
        }
    }

    Ok(report)
}

fn sync_single(
    client: &JiraClient,
    config: &PluginConfig,
    tasks: &[Task],
    local_by_key: &BTreeMap<String, &Task>,
    filter: &str,
) -> Result<SyncReport> {
    let mut report = SyncReport::default();

    if let Some(task) = tasks.iter().find(|t| t.id == filter) {
        if let Some(key) = task.remote_key() {
            let issue = client.get_issue(key)?;
            if let Some(update) = sync_diff::diff_issue(client, config, task, &issue) {
                report.updated.push(update);
            }
        }
        return Ok(report);
    }

    let issue = client.get_issue(filter)?;
    match local_by_key.get(&issue.key) {
        None => report.created.push(sync_diff::issue_to_create(client, config, &issue)),
        Some(task) => {
            if let Some(update) = sync_diff::diff_issue(client, config, task, &issue) {
                report.updated.push(update);
            }
        }
    }
    Ok(report)
}
