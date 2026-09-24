use casklet_contracts::{ApiError, CONTRACT_VERSION, CaskView, CreateCask, GetCask, ListCasks};
use clap::{Parser, Subcommand, ValueEnum};
use std::fmt::Write;
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(name = "casklet", about = "Manage your Casks")]
struct Cli {
    #[arg(long, value_enum, default_value_t = Output::Table, global = true)]
    output: Output,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Output {
    Table,
    Json,
}

#[derive(Debug, Subcommand)]
enum Command {
    Cask {
        #[command(subcommand)]
        action: CaskAction,
    },
}

#[derive(Debug, Subcommand)]
enum CaskAction {
    Create { name: String },
    List,
    Get { id: String },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let service = match casklet_infrastructure::development_service() {
        Ok(service) => service,
        Err(error) => {
            eprintln!("Could not open Casklet data: {error}");
            return ExitCode::FAILURE;
        }
    };
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let result = match cli.command {
        Command::Cask {
            action: CaskAction::Create { name },
        } => service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name,
                },
                &correlation_id,
            )
            .map(|cask| render_cask_created(&cask, cli.output)),
        Command::Cask {
            action: CaskAction::List,
        } => service
            .list(
                &ListCasks {
                    version: CONTRACT_VERSION,
                },
                &correlation_id,
            )
            .map(|casks| render_cask_list(&casks, cli.output)),
        Command::Cask {
            action: CaskAction::Get { id },
        } => service
            .get(
                &GetCask {
                    version: CONTRACT_VERSION,
                    id,
                },
                &correlation_id,
            )
            .map(|cask| render_cask_detail(&cask, cli.output)),
    };
    match result {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            render_error(&error, cli.output);
            ExitCode::FAILURE
        }
    }
}

fn render_cask_created(cask: &CaskView, output: Output) -> String {
    match output {
        Output::Table => format!("Created Cask: {}\n", cask.name),
        Output::Json => format!(
            "{}\n",
            serde_json::to_string(cask).expect("Cask DTO is serializable")
        ),
    }
}

fn render_cask_list(casks: &[CaskView], output: Output) -> String {
    match output {
        Output::Table => {
            let mut result = String::new();
            for cask in casks {
                writeln!(result, "{}", cask.name).expect("writing to a String cannot fail");
            }
            result
        }
        Output::Json => format!(
            "{}\n",
            serde_json::to_string(casks).expect("Cask DTO is serializable")
        ),
    }
}

fn render_cask_detail(cask: &CaskView, output: Output) -> String {
    match output {
        Output::Table => format!(
            "{}\nStatus: {}\n",
            cask.name,
            cask.state.to_ascii_lowercase()
        ),
        Output::Json => format!(
            "{}\n",
            serde_json::to_string(cask).expect("Cask DTO is serializable")
        ),
    }
}

fn render_error(error: &ApiError, output: Output) {
    match output {
        Output::Table => eprintln!(
            "{}: {} (reference {})",
            error.code, error.message, error.correlation_id
        ),
        Output::Json => eprintln!(
            "{}",
            serde_json::to_string(error).expect("Error DTO is serializable")
        ),
    }
}
