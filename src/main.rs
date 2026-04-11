mod auth;
mod cli;
mod commands;
mod config;
mod error;
mod jira;
mod mapping;
mod types;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::AuthSetup { auth_dir } => commands::auth_setup::run(&auth_dir),
        Command::AuthCheck { auth_dir } => commands::auth_check::run(&auth_dir),
        Command::Push {
            task,
            config,
            auth_dir,
        } => commands::push::run(&task, &config, &auth_dir),
        Command::Sync {
            task,
            config,
            auth_dir,
        } => commands::sync::run(task.as_deref(), &config, &auth_dir),
    };
    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}
