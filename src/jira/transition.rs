//! Issue transition helper, extracted from client.rs to keep file sizes
//! under the source-length limit.

use crate::config::PluginConfig;
use crate::error::{PluginError, Result};
use crate::jira::discover::DiscoveredMappings;
use crate::jira::types::TransitionsResponse;
use crate::mapping;
use serde_json::Value;

pub struct TransitionRequest<'a> {
    pub current_status: &'a str,
    pub target_status: &'a str,
    pub transitions: &'a TransitionsResponse,
    pub config: &'a PluginConfig,
    pub discovered: Option<&'a DiscoveredMappings>,
}

#[derive(Debug)]
pub enum TransitionPlan {
    /// Already in the target state, no action needed.
    NoOp,
    /// Execute the transition with this ID.
    Execute(String),
}

pub fn plan_transition(req: &TransitionRequest) -> Result<TransitionPlan> {
    let current_balls =
        mapping::jira_status_to_balls(req.current_status, req.config, req.discovered);
    let target_normalized = req.target_status.to_lowercase().replace(' ', "_");
    if current_balls == target_normalized {
        return Ok(TransitionPlan::NoOp);
    }

    let jira_status =
        mapping::balls_status_to_jira(req.target_status, req.config, req.discovered);
    let transition = req.transitions.transitions.iter().find(|t| {
        t.to.as_ref()
            .map(|to| to.name.to_lowercase() == jira_status.to_lowercase())
            .unwrap_or(false)
            || t.name.to_lowercase() == jira_status.to_lowercase()
    });

    match transition {
        Some(t) => Ok(TransitionPlan::Execute(t.id.clone())),
        None => Err(PluginError::Other(format!(
            "No Jira transition available from \"{}\" to \"{}\" (mapped to \"{}\"). \
             Available transitions: [{}]. \
             Configure status_map in plugin config if your workflow uses different names.",
            req.current_status,
            req.target_status,
            jira_status,
            available_targets(req.transitions).join(", ")
        ))),
    }
}

fn available_targets(transitions: &TransitionsResponse) -> Vec<String> {
    transitions
        .transitions
        .iter()
        .map(|t| {
            t.to.as_ref()
                .map(|to| to.name.clone())
                .unwrap_or_else(|| t.name.clone())
        })
        .collect()
}

pub fn build_transition_payload(transition_id: &str) -> Value {
    serde_json::json!({"transition": {"id": transition_id}})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_config() -> PluginConfig {
        serde_json::from_str(r#"{"url":"https://x","project":"X"}"#).unwrap()
    }

    fn transitions(items: Vec<(&str, &str, &str)>) -> TransitionsResponse {
        let json = serde_json::to_string(&serde_json::json!({
            "transitions": items.iter().map(|(id, name, target)| {
                serde_json::json!({"id": id, "name": name, "to": {"name": target}})
            }).collect::<Vec<_>>()
        })).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn plan_noop_when_already_in_state() {
        let cfg = empty_config();
        let tr = transitions(vec![("1", "Start", "In Progress")]);
        let req = TransitionRequest {
            current_status: "To Do",
            target_status: "open",
            transitions: &tr,
            config: &cfg,
            discovered: None,
        };
        assert!(matches!(plan_transition(&req).unwrap(), TransitionPlan::NoOp));
    }

    #[test]
    fn plan_execute_finds_transition() {
        let cfg = empty_config();
        let tr = transitions(vec![("42", "Start Progress", "In Progress")]);
        let req = TransitionRequest {
            current_status: "To Do",
            target_status: "in_progress",
            transitions: &tr,
            config: &cfg,
            discovered: None,
        };
        match plan_transition(&req).unwrap() {
            TransitionPlan::Execute(id) => assert_eq!(id, "42"),
            _ => panic!("expected Execute"),
        }
    }

    #[test]
    fn plan_errors_when_no_match() {
        let cfg = empty_config();
        let tr = transitions(vec![("1", "Start", "Something")]);
        let req = TransitionRequest {
            current_status: "To Do",
            target_status: "closed",
            transitions: &tr,
            config: &cfg,
            discovered: None,
        };
        let err = plan_transition(&req).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("No Jira transition"));
        assert!(msg.contains("Something"));
    }

    #[test]
    fn build_payload() {
        let p = build_transition_payload("99");
        assert_eq!(p["transition"]["id"], "99");
    }
}
