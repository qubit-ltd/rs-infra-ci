use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use qubit_infra_ci::{Task, load_config, plan_workflow, run};

#[derive(Debug, Parser)]
#[command(name = "rs-infra-ci", about = "Run the project's reusable CI tasks")]
struct Cli {
    #[arg(long, default_value = ".")]
    project: PathBuf,
    #[arg(long, value_delimiter = ',')]
    only: Vec<Task>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Plan,
    Check,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = load_config(&cli.project)?;
    let tasks = config.select(&cli.only)?;

    match cli.command {
        Command::Plan => plan_workflow(&cli.project, &tasks),
        Command::Check => run(&cli.project, tasks),
    }
}
