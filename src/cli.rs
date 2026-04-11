use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "balls-plugin-jira")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Interactive auth setup
    #[command(name = "auth-setup")]
    AuthSetup {
        #[arg(long)]
        auth_dir: PathBuf,
    },
    /// Check if auth is still valid
    #[command(name = "auth-check")]
    AuthCheck {
        #[arg(long)]
        auth_dir: PathBuf,
    },
    /// Push a task to Jira
    Push {
        #[arg(long)]
        task: String,
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        auth_dir: PathBuf,
    },
    /// Sync tasks with Jira
    Sync {
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
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
