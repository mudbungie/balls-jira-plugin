use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "balls-plugin-jira",
    about = "Jira integration plugin for balls (bidirectional sync)",
    long_about = "A balls plugin that synchronizes tasks with Jira issues. \
                  Supports Jira Cloud and Server/Data Center, with PAT, OAuth 2.0, \
                  and SAML device-auth authentication."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Interactive authentication setup: prompts for credentials and stores them in --auth-dir.
    #[command(name = "auth-setup")]
    AuthSetup {
        /// Directory where credentials will be stored (created if missing).
        #[arg(long, value_name = "DIR")]
        auth_dir: PathBuf,
    },

    /// Verify that stored credentials are valid. Exit 0 if OK, non-zero otherwise.
    #[command(name = "auth-check")]
    AuthCheck {
        /// Directory containing credentials written by auth-setup.
        #[arg(long, value_name = "DIR")]
        auth_dir: PathBuf,
    },

    /// Push a single task to Jira. Reads the Task JSON on stdin; writes a PushResponse on stdout.
    Push {
        /// balls task ID (used to cross-check the Task read from stdin).
        #[arg(long, value_name = "ID")]
        task: String,
        /// Path to the plugin config file (e.g. .balls/plugins/jira.json).
        #[arg(long, value_name = "FILE")]
        config: PathBuf,
        /// Directory containing credentials written by auth-setup.
        #[arg(long, value_name = "DIR")]
        auth_dir: PathBuf,
    },

    /// Sync tasks with Jira. Reads an array of Task JSON on stdin; writes a SyncReport on stdout.
    Sync {
        /// Optional task filter (balls ID like bl-a1b2 or Jira key like PROJ-123).
        #[arg(long, value_name = "ID")]
        task: Option<String>,
        /// Path to the plugin config file (e.g. .balls/plugins/jira.json).
        #[arg(long, value_name = "FILE")]
        config: PathBuf,
        /// Directory containing credentials written by auth-setup.
        #[arg(long, value_name = "DIR")]
        auth_dir: PathBuf,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_parses() {
        Cli::command().debug_assert();
    }
}
