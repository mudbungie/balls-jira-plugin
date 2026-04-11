use crate::config::PluginConfig;
use crate::jira::discover::{self, DiscoveredMappings};
use std::collections::HashMap;

/// Hardcoded fallbacks — used only when both config and discovery are empty.
const FALLBACK_STATUS_MAP: &[(&str, &str)] = &[
    ("open", "To Do"),
    ("in_progress", "In Progress"),
    ("review", "In Review"),
    ("blocked", "Blocked"),
    ("closed", "Done"),
    ("deferred", "Backlog"),
];

const FALLBACK_PRIORITY: &[(u8, &str)] = &[
    (1, "Highest"),
    (2, "High"),
    (3, "Medium"),
    (4, "Low"),
];

const FALLBACK_TYPES: &[(&str, &str)] = &[
    ("epic", "Epic"),
    ("task", "Task"),
    ("bug", "Bug"),
];

/// balls status → Jira status name.
/// Precedence: config.status_map > discovered > fallback.
pub fn balls_status_to_jira(
    status: &str,
    config: &PluginConfig,
    discovered: Option<&DiscoveredMappings>,
) -> String {
    let normalized = status.to_lowercase().replace(' ', "_");
    if let Some(mapped) = config.status_map.get(&normalized) {
        return mapped.clone();
    }
    if let Some(d) = discovered {
        let auto_map = discover::build_status_map(&d.statuses);
        if let Some(mapped) = auto_map.get(&normalized) {
            return mapped.clone();
        }
    }
    for (balls, jira) in FALLBACK_STATUS_MAP {
        if *balls == normalized {
            return (*jira).to_string();
        }
    }
    status.to_string()
}

/// Jira status name → balls status.
/// Precedence: config.status_map (reversed) > discovered (by category) > fallback.
pub fn jira_status_to_balls(
    jira_status: &str,
    config: &PluginConfig,
    discovered: Option<&DiscoveredMappings>,
) -> String {
    let reverse = reverse_map(&config.status_map);
    let lower = jira_status.to_lowercase();
    if let Some(balls) = reverse.get(&lower) {
        return balls.clone();
    }
    if let Some(d) = discovered {
        if let Some(cat) = d.statuses.get(jira_status) {
            return discover::category_to_balls_status(cat).to_string();
        }
        for (name, cat) in &d.statuses {
            if name.to_lowercase() == lower {
                return discover::category_to_balls_status(cat).to_string();
            }
        }
    }
    for (balls, jira) in FALLBACK_STATUS_MAP {
        if jira.to_lowercase() == lower {
            return (*balls).to_string();
        }
    }
    "open".to_string()
}

/// balls priority (1-4) → Jira priority name.
pub fn balls_priority_to_jira(priority: u8, discovered: Option<&DiscoveredMappings>) -> String {
    if let Some(d) = discovered {
        if let Some(name) = discover::priority_balls_to_jira(priority, &d.priorities) {
            return name;
        }
    }
    for (p, name) in FALLBACK_PRIORITY {
        if *p == priority {
            return (*name).to_string();
        }
    }
    "Medium".to_string()
}

/// Jira priority name → balls priority (1-4).
pub fn jira_priority_to_balls(jira_priority: &str, discovered: Option<&DiscoveredMappings>) -> u8 {
    if let Some(d) = discovered {
        if let Some(p) = discover::priority_jira_to_balls(jira_priority, &d.priorities) {
            return p;
        }
    }
    let lower = jira_priority.to_lowercase();
    for (p, name) in FALLBACK_PRIORITY {
        if name.to_lowercase() == lower {
            return *p;
        }
    }
    3
}

pub fn balls_type_to_jira(task_type: &str) -> String {
    let lower = task_type.to_lowercase();
    for (balls, jira) in FALLBACK_TYPES {
        if *balls == lower {
            return (*jira).to_string();
        }
    }
    "Task".to_string()
}

pub fn jira_type_to_balls(jira_type: &str) -> String {
    let lower = jira_type.to_lowercase();
    for (balls, jira) in FALLBACK_TYPES {
        if jira.to_lowercase() == lower {
            return (*balls).to_string();
        }
    }
    if lower == "story" {
        return "task".to_string();
    }
    "task".to_string()
}

fn reverse_map(map: &HashMap<String, String>) -> HashMap<String, String> {
    map.iter()
        .map(|(k, v)| (v.to_lowercase(), k.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_config() -> PluginConfig {
        serde_json::from_str(r#"{"url":"https://x","project":"X"}"#).unwrap()
    }

    fn custom_config() -> PluginConfig {
        serde_json::from_str(r#"{
            "url":"https://x","project":"X",
            "status_map":{"open":"Nuevo","closed":"Cerrado"}
        }"#)
        .unwrap()
    }

    fn mock_discovered() -> DiscoveredMappings {
        let mut statuses = HashMap::new();
        statuses.insert("Backlog".into(), "new".into());
        statuses.insert("Developing".into(), "indeterminate".into());
        statuses.insert("Finished".into(), "done".into());
        DiscoveredMappings {
            priorities: vec!["P0".into(), "P1".into(), "P2".into(), "P3".into()],
            statuses,
            ..Default::default()
        }
    }

    // --- status tests ---

    #[test]
    fn status_fallback() {
        let cfg = empty_config();
        assert_eq!(balls_status_to_jira("open", &cfg, None), "To Do");
        assert_eq!(balls_status_to_jira("closed", &cfg, None), "Done");
    }

    #[test]
    fn status_discovered_overrides_fallback() {
        let cfg = empty_config();
        let d = mock_discovered();
        assert_eq!(balls_status_to_jira("open", &cfg, Some(&d)), "Backlog");
        assert_eq!(balls_status_to_jira("closed", &cfg, Some(&d)), "Finished");
    }

    #[test]
    fn status_config_overrides_discovered() {
        let cfg = custom_config();
        let d = mock_discovered();
        assert_eq!(balls_status_to_jira("open", &cfg, Some(&d)), "Nuevo");
        assert_eq!(balls_status_to_jira("closed", &cfg, Some(&d)), "Cerrado");
    }

    #[test]
    fn status_unknown_passthrough() {
        let cfg = empty_config();
        assert_eq!(balls_status_to_jira("mystery", &cfg, None), "mystery");
    }

    #[test]
    fn reverse_status_fallback() {
        let cfg = empty_config();
        assert_eq!(jira_status_to_balls("To Do", &cfg, None), "open");
        assert_eq!(jira_status_to_balls("Done", &cfg, None), "closed");
    }

    #[test]
    fn reverse_status_discovered() {
        let cfg = empty_config();
        let d = mock_discovered();
        assert_eq!(jira_status_to_balls("Backlog", &cfg, Some(&d)), "open");
        assert_eq!(jira_status_to_balls("Developing", &cfg, Some(&d)), "in_progress");
        assert_eq!(jira_status_to_balls("Finished", &cfg, Some(&d)), "closed");
    }

    #[test]
    fn reverse_status_unknown() {
        let cfg = empty_config();
        assert_eq!(jira_status_to_balls("Weird", &cfg, None), "open");
    }

    // --- priority tests ---

    #[test]
    fn priority_fallback() {
        assert_eq!(balls_priority_to_jira(1, None), "Highest");
        assert_eq!(balls_priority_to_jira(4, None), "Low");
        assert_eq!(balls_priority_to_jira(5, None), "Medium");
    }

    #[test]
    fn priority_discovered() {
        let d = mock_discovered();
        assert_eq!(balls_priority_to_jira(1, Some(&d)), "P0");
        assert_eq!(balls_priority_to_jira(4, Some(&d)), "P3");
    }

    #[test]
    fn reverse_priority_discovered() {
        let d = mock_discovered();
        assert_eq!(jira_priority_to_balls("P1", Some(&d)), 2);
        assert_eq!(jira_priority_to_balls("Unknown", Some(&d)), 3);
    }

    #[test]
    fn reverse_priority_fallback() {
        assert_eq!(jira_priority_to_balls("Highest", None), 1);
        assert_eq!(jira_priority_to_balls("Low", None), 4);
        assert_eq!(jira_priority_to_balls("Unknown", None), 3);
    }

    // --- type tests ---

    #[test]
    fn type_to_jira() {
        assert_eq!(balls_type_to_jira("epic"), "Epic");
        assert_eq!(balls_type_to_jira("task"), "Task");
        assert_eq!(balls_type_to_jira("bug"), "Bug");
        assert_eq!(balls_type_to_jira("unknown"), "Task");
    }

    #[test]
    fn type_from_jira() {
        assert_eq!(jira_type_to_balls("Epic"), "epic");
        assert_eq!(jira_type_to_balls("Task"), "task");
        assert_eq!(jira_type_to_balls("Bug"), "bug");
        assert_eq!(jira_type_to_balls("Story"), "task");
        assert_eq!(jira_type_to_balls("Unknown"), "task");
    }
}
