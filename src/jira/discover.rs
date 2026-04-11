use crate::config::ServerType;
use crate::error::Result;
use serde::Deserialize;
use std::collections::HashMap;

/// Mappings discovered from the live Jira instance.
#[derive(Debug, Clone, Default)]
pub struct DiscoveredMappings {
    pub server_type: Option<ServerType>,
    /// Jira status name → statusCategory key ("new", "indeterminate", "done")
    pub statuses: HashMap<String, String>,
    /// Ordered list of priority names (index 0 = highest)
    pub priorities: Vec<String>,
    /// Jira issue type names available in the project
    pub issue_types: Vec<String>,
}

// --- serverInfo ---

#[derive(Debug, Deserialize)]
struct ServerInfo {
    #[serde(rename = "deploymentType")]
    deployment_type: Option<String>,
    #[serde(rename = "versionNumbers", default)]
    version_numbers: Vec<u64>,
}

pub fn parse_server_type(body: &str) -> Option<ServerType> {
    let info: ServerInfo = serde_json::from_str(body).ok()?;
    if let Some(dt) = &info.deployment_type {
        if dt.eq_ignore_ascii_case("cloud") {
            return Some(ServerType::Cloud);
        }
        if dt.eq_ignore_ascii_case("server") || dt.eq_ignore_ascii_case("datacenter") {
            return Some(ServerType::Server);
        }
    }
    if info.version_numbers.first().copied().unwrap_or(0) >= 1000 {
        return Some(ServerType::Cloud);
    }
    None
}

// --- project statuses ---

#[derive(Debug, Deserialize)]
struct IssueTypeStatuses {
    #[serde(default)]
    statuses: Vec<JiraStatusEntry>,
}

#[derive(Debug, Deserialize)]
struct JiraStatusEntry {
    name: String,
    #[serde(rename = "statusCategory", default)]
    status_category: Option<StatusCategory>,
}

#[derive(Debug, Deserialize)]
struct StatusCategory {
    key: String,
}

pub fn parse_project_statuses(body: &str) -> HashMap<String, String> {
    let entries: Vec<IssueTypeStatuses> = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };
    let mut map = HashMap::new();
    for it in entries {
        for s in it.statuses {
            let cat = s
                .status_category
                .map(|c| c.key)
                .unwrap_or_else(|| "undefined".into());
            map.entry(s.name).or_insert(cat);
        }
    }
    map
}

// --- priorities ---

#[derive(Debug, Deserialize)]
struct JiraPriorityEntry {
    name: String,
}

pub fn parse_priorities(body: &str) -> Vec<String> {
    let entries: Vec<JiraPriorityEntry> = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    entries.into_iter().map(|p| p.name).collect()
}

// --- issue types ---

#[derive(Debug, Deserialize)]
struct JiraIssueTypeEntry {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ProjectDetail {
    #[serde(rename = "issueTypes", default)]
    issue_types: Vec<JiraIssueTypeEntry>,
}

pub fn parse_issue_types(body: &str) -> Vec<String> {
    // Try as flat array first (Cloud /issuetype or Server /issuetype)
    if let Ok(entries) = serde_json::from_str::<Vec<JiraIssueTypeEntry>>(body) {
        return entries.into_iter().map(|t| t.name).collect();
    }
    // Try as project detail (Server /project/{key})
    if let Ok(detail) = serde_json::from_str::<ProjectDetail>(body) {
        return detail.issue_types.into_iter().map(|t| t.name).collect();
    }
    Vec::new()
}

/// Map a Jira statusCategory key to a balls status.
pub fn category_to_balls_status(category_key: &str) -> &'static str {
    match category_key {
        "new" => "open",
        "indeterminate" => "in_progress",
        "done" => "closed",
        _ => "open",
    }
}

/// Build a status map from discovered statuses: balls_status → jira_status_name.
/// Groups by category, picks the first Jira status name for each balls status.
pub fn build_status_map(statuses: &HashMap<String, String>) -> HashMap<String, String> {
    let mut result: HashMap<String, String> = HashMap::new();
    for (jira_name, category_key) in statuses {
        let balls = category_to_balls_status(category_key);
        result.entry(balls.to_string()).or_insert_with(|| jira_name.clone());
    }
    result
}

/// Map a balls priority (1-4) to a Jira priority name using discovered list.
/// Jira returns priorities in order (highest first).
pub fn priority_balls_to_jira(priority: u8, discovered: &[String]) -> Option<String> {
    if discovered.is_empty() {
        return None;
    }
    let idx = (priority as usize).saturating_sub(1).min(discovered.len() - 1);
    Some(discovered[idx].clone())
}

/// Map a Jira priority name to a balls priority using discovered list.
pub fn priority_jira_to_balls(jira_name: &str, discovered: &[String]) -> Option<u8> {
    let lower = jira_name.to_lowercase();
    for (i, name) in discovered.iter().enumerate() {
        if name.to_lowercase() == lower {
            return Some((i as u8) + 1);
        }
    }
    None
}

/// Discover all mappings by querying the Jira API.
pub fn run_discovery(
    get: &dyn Fn(&str) -> Result<String>,
    project_key: &str,
) -> DiscoveredMappings {
    let mut m = DiscoveredMappings::default();

    if let Ok(body) = get("/serverInfo") {
        m.server_type = parse_server_type(&body);
    }
    let status_path = format!("/project/{}/statuses", project_key);
    if let Ok(body) = get(&status_path) {
        m.statuses = parse_project_statuses(&body);
    }
    if let Ok(body) = get("/priority") {
        m.priorities = parse_priorities(&body);
    }
    if let Ok(body) = get("/issuetype") {
        m.issue_types = parse_issue_types(&body);
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_type_cloud() {
        let body = r#"{"deploymentType":"Cloud","versionNumbers":[1001,0,0]}"#;
        assert_eq!(parse_server_type(body), Some(ServerType::Cloud));
    }

    #[test]
    fn server_type_server() {
        let body = r#"{"deploymentType":"Server","versionNumbers":[9,4,0]}"#;
        assert_eq!(parse_server_type(body), Some(ServerType::Server));
    }

    #[test]
    fn server_type_datacenter() {
        let body = r#"{"deploymentType":"DataCenter"}"#;
        assert_eq!(parse_server_type(body), Some(ServerType::Server));
    }

    #[test]
    fn server_type_by_version() {
        let body = r#"{"versionNumbers":[1001,0,0]}"#;
        assert_eq!(parse_server_type(body), Some(ServerType::Cloud));
    }

    #[test]
    fn server_type_unknown() {
        assert_eq!(parse_server_type("{}"), None);
        assert_eq!(parse_server_type("bad json"), None);
    }

    #[test]
    fn statuses_from_json() {
        let body = r#"[{"id":"1","name":"Task","statuses":[
            {"name":"To Do","statusCategory":{"key":"new"}},
            {"name":"Done","statusCategory":{"key":"done"}}
        ]},{"id":"2","name":"Bug","statuses":[
            {"name":"In Progress","statusCategory":{"key":"indeterminate"}}
        ]}]"#;
        let map = parse_project_statuses(body);
        assert_eq!(map.get("To Do").unwrap(), "new");
        assert_eq!(map.get("Done").unwrap(), "done");
        assert_eq!(map.get("In Progress").unwrap(), "indeterminate");
        assert!(parse_project_statuses("bad").is_empty());
    }

    #[test]
    fn priorities_from_json() {
        let body = r#"[{"name":"Highest"},{"name":"High"},{"name":"Medium"},{"name":"Low"}]"#;
        assert_eq!(parse_priorities(body), vec!["Highest", "High", "Medium", "Low"]);
        assert!(parse_priorities("bad").is_empty());
    }

    #[test]
    fn issue_types_from_json() {
        let arr = r#"[{"name":"Epic"},{"name":"Story"},{"name":"Bug"}]"#;
        assert_eq!(parse_issue_types(arr), vec!["Epic", "Story", "Bug"]);
        let proj = r#"{"issueTypes":[{"name":"Task"},{"name":"Bug"}]}"#;
        assert_eq!(parse_issue_types(proj), vec!["Task", "Bug"]);
    }

    #[test]
    fn build_status_map_from_categories() {
        let mut statuses = HashMap::new();
        statuses.insert("Backlog".into(), "new".into());
        statuses.insert("In Dev".into(), "indeterminate".into());
        statuses.insert("Closed".into(), "done".into());
        let map = build_status_map(&statuses);
        assert_eq!(map.get("open").unwrap(), "Backlog");
        assert_eq!(map.get("in_progress").unwrap(), "In Dev");
        assert_eq!(map.get("closed").unwrap(), "Closed");
    }

    #[test]
    fn priority_mapping_discovered() {
        let discovered = vec!["P0".into(), "P1".into(), "P2".into(), "P3".into()];
        assert_eq!(priority_balls_to_jira(1, &discovered), Some("P0".into()));
        assert_eq!(priority_balls_to_jira(4, &discovered), Some("P3".into()));
        assert_eq!(priority_balls_to_jira(5, &discovered), Some("P3".into()));
        assert_eq!(priority_jira_to_balls("P1", &discovered), Some(2));
        assert_eq!(priority_jira_to_balls("Nope", &discovered), None);
    }

    #[test]
    fn priority_mapping_empty() {
        assert_eq!(priority_balls_to_jira(1, &[]), None);
        assert_eq!(priority_jira_to_balls("X", &[]), None);
    }

    #[test]
    fn category_mapping() {
        assert_eq!(category_to_balls_status("new"), "open");
        assert_eq!(category_to_balls_status("indeterminate"), "in_progress");
        assert_eq!(category_to_balls_status("done"), "closed");
        assert_eq!(category_to_balls_status("undefined"), "open");
    }

    #[test]
    fn run_discovery_handles_failures() {
        let get = |_path: &str| -> Result<String> {
            Err(crate::error::PluginError::Other("offline".into()))
        };
        let m = run_discovery(&get, "X");
        assert!(m.server_type.is_none());
        assert!(m.statuses.is_empty());
    }
}
