use crate::config::PluginConfig;
use std::collections::HashMap;

const DEFAULT_STATUS_MAP: &[(&str, &str)] = &[
    ("open", "To Do"),
    ("in_progress", "In Progress"),
    ("review", "In Review"),
    ("blocked", "Blocked"),
    ("closed", "Done"),
    ("deferred", "Backlog"),
];

const DEFAULT_PRIORITY_MAP: &[(u8, &str)] = &[
    (1, "Highest"),
    (2, "High"),
    (3, "Medium"),
    (4, "Low"),
];

const DEFAULT_TYPE_MAP: &[(&str, &str)] = &[
    ("epic", "Epic"),
    ("task", "Task"),
    ("bug", "Bug"),
];

pub fn balls_status_to_jira(status: &str, config: &PluginConfig) -> String {
    let normalized = status.to_lowercase().replace(' ', "_");
    if let Some(mapped) = config.status_map.get(&normalized) {
        return mapped.clone();
    }
    for (balls, jira) in DEFAULT_STATUS_MAP {
        if *balls == normalized {
            return (*jira).to_string();
        }
    }
    status.to_string()
}

pub fn jira_status_to_balls(jira_status: &str, config: &PluginConfig) -> String {
    let reverse = reverse_map(&config.status_map);
    let lower = jira_status.to_lowercase();
    if let Some(balls) = reverse.get(&lower) {
        return balls.clone();
    }
    for (balls, jira) in DEFAULT_STATUS_MAP {
        if jira.to_lowercase() == lower {
            return (*balls).to_string();
        }
    }
    "open".to_string()
}

pub fn balls_priority_to_jira(priority: u8) -> &'static str {
    for (p, name) in DEFAULT_PRIORITY_MAP {
        if *p == priority {
            return name;
        }
    }
    "Medium"
}

pub fn jira_priority_to_balls(jira_priority: &str) -> u8 {
    let lower = jira_priority.to_lowercase();
    for (p, name) in DEFAULT_PRIORITY_MAP {
        if name.to_lowercase() == lower {
            return *p;
        }
    }
    3
}

pub fn balls_type_to_jira(task_type: &str) -> String {
    let lower = task_type.to_lowercase();
    for (balls, jira) in DEFAULT_TYPE_MAP {
        if *balls == lower {
            return (*jira).to_string();
        }
    }
    "Task".to_string()
}

pub fn jira_type_to_balls(jira_type: &str) -> String {
    let lower = jira_type.to_lowercase();
    for (balls, jira) in DEFAULT_TYPE_MAP {
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

    #[test]
    fn status_default_mapping() {
        let cfg = empty_config();
        assert_eq!(balls_status_to_jira("open", &cfg), "To Do");
        assert_eq!(balls_status_to_jira("in_progress", &cfg), "In Progress");
        assert_eq!(balls_status_to_jira("review", &cfg), "In Review");
        assert_eq!(balls_status_to_jira("blocked", &cfg), "Blocked");
        assert_eq!(balls_status_to_jira("closed", &cfg), "Done");
        assert_eq!(balls_status_to_jira("deferred", &cfg), "Backlog");
    }

    #[test]
    fn status_custom_mapping() {
        let cfg = custom_config();
        assert_eq!(balls_status_to_jira("open", &cfg), "Nuevo");
        assert_eq!(balls_status_to_jira("closed", &cfg), "Cerrado");
        // Uncustomized falls through to default
        assert_eq!(balls_status_to_jira("blocked", &cfg), "Blocked");
    }

    #[test]
    fn status_unknown_passthrough() {
        let cfg = empty_config();
        assert_eq!(balls_status_to_jira("mystery", &cfg), "mystery");
    }

    #[test]
    fn status_case_insensitive() {
        let cfg = empty_config();
        assert_eq!(balls_status_to_jira("Open", &cfg), "To Do");
        assert_eq!(balls_status_to_jira("In Progress", &cfg), "In Progress");
    }

    #[test]
    fn reverse_status_default() {
        let cfg = empty_config();
        assert_eq!(jira_status_to_balls("To Do", &cfg), "open");
        assert_eq!(jira_status_to_balls("In Progress", &cfg), "in_progress");
        assert_eq!(jira_status_to_balls("Done", &cfg), "closed");
    }

    #[test]
    fn reverse_status_custom() {
        let cfg = custom_config();
        assert_eq!(jira_status_to_balls("Nuevo", &cfg), "open");
        assert_eq!(jira_status_to_balls("Cerrado", &cfg), "closed");
    }

    #[test]
    fn reverse_status_unknown() {
        let cfg = empty_config();
        assert_eq!(jira_status_to_balls("Weird", &cfg), "open");
    }

    #[test]
    fn reverse_status_case_insensitive() {
        let cfg = empty_config();
        assert_eq!(jira_status_to_balls("to do", &cfg), "open");
        assert_eq!(jira_status_to_balls("TO DO", &cfg), "open");
    }

    #[test]
    fn priority_to_jira() {
        assert_eq!(balls_priority_to_jira(1), "Highest");
        assert_eq!(balls_priority_to_jira(2), "High");
        assert_eq!(balls_priority_to_jira(3), "Medium");
        assert_eq!(balls_priority_to_jira(4), "Low");
        assert_eq!(balls_priority_to_jira(5), "Medium");
    }

    #[test]
    fn priority_from_jira() {
        assert_eq!(jira_priority_to_balls("Highest"), 1);
        assert_eq!(jira_priority_to_balls("High"), 2);
        assert_eq!(jira_priority_to_balls("Medium"), 3);
        assert_eq!(jira_priority_to_balls("Low"), 4);
        assert_eq!(jira_priority_to_balls("Unknown"), 3);
    }

    #[test]
    fn priority_case_insensitive() {
        assert_eq!(jira_priority_to_balls("highest"), 1);
        assert_eq!(jira_priority_to_balls("LOW"), 4);
    }

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

    #[test]
    fn type_case_insensitive() {
        assert_eq!(balls_type_to_jira("EPIC"), "Epic");
        assert_eq!(jira_type_to_balls("bug"), "bug");
    }
}
