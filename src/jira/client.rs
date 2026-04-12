use crate::auth::AuthProvider;
use crate::config::{PluginConfig, ServerType};
use crate::error::{PluginError, Result};
use crate::jira::transition::{self, TransitionPlan, TransitionRequest};
use crate::jira::{cloud, discover, server, types::*};
use reqwest::blocking::Client;
use std::path::Path;

pub struct JiraClient<'a> {
    http: Client,
    config: PluginConfig,
    auth: &'a dyn AuthProvider,
    auth_dir: &'a Path,
}

impl<'a> JiraClient<'a> {
    pub fn new(
        config: PluginConfig,
        auth: &'a dyn AuthProvider,
        auth_dir: &'a Path,
    ) -> Self {
        Self {
            http: Client::new(),
            config,
            auth,
            auth_dir,
        }
    }

    fn api_url(&self, path: &str) -> String {
        format!(
            "{}{}{}",
            self.config.url.trim_end_matches('/'),
            self.config.api_base(),
            path
        )
    }

    fn get(&self, path: &str) -> Result<reqwest::blocking::RequestBuilder> {
        let url = self.api_url(path);
        let builder = self.http.get(&url);
        self.auth.authenticate(self.auth_dir, builder)
    }

    fn post(&self, path: &str) -> Result<reqwest::blocking::RequestBuilder> {
        let url = self.api_url(path);
        let builder = self.http.post(&url).header("Content-Type", "application/json");
        self.auth.authenticate(self.auth_dir, builder)
    }

    fn put(&self, path: &str) -> Result<reqwest::blocking::RequestBuilder> {
        let url = self.api_url(path);
        let builder = self.http.put(&url).header("Content-Type", "application/json");
        self.auth.authenticate(self.auth_dir, builder)
    }

    fn check_response(resp: reqwest::blocking::Response) -> Result<reqwest::blocking::Response> {
        let status = resp.status();
        if status.is_success() {
            Ok(resp)
        } else {
            let code = status.as_u16();
            let body = resp.text().unwrap_or_default();
            Err(PluginError::JiraApi { status: code, body })
        }
    }

    pub fn create_issue(&self, fields: &CreateFields) -> Result<(String, String)> {
        let payload = match *self.config.effective_server_type() {
            ServerType::Cloud => cloud::build_create_payload(
                &fields.project_key,
                &fields.summary,
                &fields.issuetype,
                fields.description.as_ref().and_then(|d| d.as_str()),
                fields.priority.as_deref(),
                &fields.labels,
            ),
            ServerType::Server => server::build_create_payload(
                &fields.project_key,
                &fields.summary,
                &fields.issuetype,
                fields.description.as_ref().and_then(|d| d.as_str()),
                fields.priority.as_deref(),
                &fields.labels,
            ),
        };
        let resp = self.post("/issue")?.json(&payload).send()?;
        let resp = Self::check_response(resp)?;
        let cr: CreateIssueResponse = resp.json()?;
        let browse_url = format!(
            "{}/browse/{}",
            self.config.url.trim_end_matches('/'),
            cr.key
        );
        Ok((cr.key, browse_url))
    }

    pub fn update_issue(&self, key: &str, fields: &UpdateFields) -> Result<()> {
        if fields.is_empty() {
            return Ok(());
        }
        let payload = match *self.config.effective_server_type() {
            ServerType::Cloud => cloud::build_update_payload(
                fields.summary.as_deref(),
                fields.description.as_ref().and_then(|d| d.as_str()),
                fields.priority.as_deref(),
                fields.labels.as_deref(),
            ),
            ServerType::Server => server::build_update_payload(
                fields.summary.as_deref(),
                fields.description.as_ref().and_then(|d| d.as_str()),
                fields.priority.as_deref(),
                fields.labels.as_deref(),
            ),
        };
        let path = format!("/issue/{}", key);
        let resp = self.put(&path)?.json(&payload).send()?;
        Self::check_response(resp)?;
        Ok(())
    }

    pub fn transition_issue(
        &self,
        key: &str,
        target_status: &str,
        config: &PluginConfig,
        discovered: Option<&discover::DiscoveredMappings>,
    ) -> Result<()> {
        let path = format!("/issue/{}/transitions", key);
        let resp = self.get(&path)?.send()?;
        let tr: TransitionsResponse = Self::check_response(resp)?.json()?;
        let current = self.get_issue(key)?;

        let plan = transition::plan_transition(&TransitionRequest {
            current_status: &current.fields.status.name,
            target_status,
            transitions: &tr,
            config,
            discovered,
        })?;
        let id = match plan {
            TransitionPlan::NoOp => return Ok(()),
            TransitionPlan::Execute(id) => id,
        };

        let payload = transition::build_transition_payload(&id);
        let resp = self.post(&path)?.json(&payload).send()?;
        Self::check_response(resp)?;
        Ok(())
    }

    pub fn get_issue(&self, key: &str) -> Result<JiraIssue> {
        let path = format!("/issue/{}", key);
        let resp = self.get(&path)?.send()?;
        let resp = Self::check_response(resp)?;
        Ok(resp.json()?)
    }

    pub fn search(&self, jql: &str) -> Result<Vec<JiraIssue>> {
        let mut all = Vec::new();
        let mut start_at = 0u64;
        loop {
            let resp = self
                .get("/search")?
                .query(&[
                    ("jql", jql),
                    ("startAt", &start_at.to_string()),
                    ("maxResults", "100"),
                ])
                .send()?;
            let resp = Self::check_response(resp)?;
            let sr: SearchResults = resp.json()?;
            let count = sr.issues.len() as u64;
            all.extend(sr.issues);
            if start_at + count >= sr.total {
                break;
            }
            start_at += count;
        }
        Ok(all)
    }

    pub fn extract_description(&self, issue: &JiraIssue) -> String {
        match &issue.fields.description {
            None => String::new(),
            Some(desc) => match *self.config.effective_server_type() {
                ServerType::Cloud => cloud::adf_to_text(desc),
                ServerType::Server => server::extract_description(desc),
            },
        }
    }

    pub fn browse_url(&self, key: &str) -> String {
        format!("{}/browse/{}", self.config.url.trim_end_matches('/'), key)
    }

    /// Query the Jira instance to discover server type, statuses, priorities,
    /// and issue types. Updates server_type in-place if auto-detected.
    /// Fails with an actionable error if server_type is in auto mode and
    /// detection failed, so we don't silently send Cloud v3 requests to a
    /// Server v2 instance.
    pub fn discover(&mut self) -> Result<discover::DiscoveredMappings> {
        let get = |path: &str| -> Result<String> {
            let resp = self.get(path)?.send()?;
            let resp = Self::check_response(resp)?;
            Ok(resp.text()?)
        };
        let mut m = discover::run_discovery(&get, &self.config.project.clone());
        match (&m.server_type, self.config.server_type_auto()) {
            (Some(st), true) => self.config.server_type = Some(st.clone()),
            (None, true) => {
                return Err(PluginError::Config(
                    "Could not auto-detect server_type from /serverInfo. \
                     Set \"server_type\": \"cloud\" or \"server\" in the \
                     plugin config explicitly."
                        .into(),
                ));
            }
            _ => {}
        }
        if m.server_type.is_none() {
            m.server_type = Some(self.config.effective_server_type().clone());
        }
        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockAuth;
    impl AuthProvider for MockAuth {
        fn setup(&self, _: &Path) -> Result<()> {
            Ok(())
        }
        fn check(&self, _: &Path) -> Result<()> {
            Ok(())
        }
        fn authenticate(&self, _: &Path, b: reqwest::blocking::RequestBuilder) -> Result<reqwest::blocking::RequestBuilder> {
            Ok(b.header("Authorization", "Bearer mock"))
        }
    }

    fn make_client() -> JiraClient<'static> {
        static AUTH: MockAuth = MockAuth;
        static AUTH_DIR: &str = "/tmp";
        let config: PluginConfig = serde_json::from_str(
            r#"{"url":"https://jira.example.com","project":"TEST"}"#,
        )
        .unwrap();
        JiraClient::new(config, &AUTH, Path::new(AUTH_DIR))
    }

    #[test]
    fn api_url_cloud() {
        let c = make_client();
        assert_eq!(
            c.api_url("/myself"),
            "https://jira.example.com/rest/api/3/myself"
        );
    }

    #[test]
    fn browse_url_format() {
        let c = make_client();
        assert_eq!(
            c.browse_url("TEST-1"),
            "https://jira.example.com/browse/TEST-1"
        );
    }

    #[test]
    fn extract_description_cloud_null() {
        let c = make_client();
        let issue: JiraIssue = serde_json::from_str(r#"{
            "key": "T-1",
            "fields": {"summary":"s","issuetype":{"name":"Task"},"status":{"name":"Open"},"description":null}
        }"#).unwrap();
        assert_eq!(c.extract_description(&issue), "");
    }
}
