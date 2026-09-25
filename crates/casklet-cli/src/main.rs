use casklet_contracts::{
    AddMode, AddResource, ApiError, CONTRACT_VERSION, CancelOperation, CaskView,
    CategorizeResource, CategoryView, CreateCask, CreateCategory, CreateDiagnosticsBundle, GetCask,
    GetOperation, GetResource, GetStorageSummary, ListCasks, ListCategories, ListOperations,
    ListResources, OperationView, PreserveOperation, RecoverOperation, RecoveryInspection,
    RecoveryStatus, ResourceView, StorageSummary,
};
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
    Resource {
        #[command(subcommand)]
        action: ResourceAction,
    },
    Category {
        #[command(subcommand)]
        action: CategoryAction,
    },
    Storage {
        #[command(subcommand)]
        action: StorageAction,
    },
    Operation {
        #[command(subcommand)]
        action: OperationAction,
    },
    Recovery {
        #[command(subcommand)]
        action: RecoveryAction,
    },
    Diagnostics {
        #[command(subcommand)]
        action: DiagnosticsAction,
    },
}

#[derive(Debug, Subcommand)]
enum CaskAction {
    Create { name: String },
    List,
    Get { id: String },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Mode {
    Copy,
    Move,
    Reference,
}
impl From<Mode> for AddMode {
    fn from(value: Mode) -> Self {
        match value {
            Mode::Copy => Self::Copy,
            Mode::Move => Self::Move,
            Mode::Reference => Self::Reference,
        }
    }
}

#[derive(Debug, Subcommand)]
enum ResourceAction {
    Add {
        cask_id: String,
        source_path: String,
        #[arg(long, value_enum)]
        mode: Mode,
        #[arg(long, default_value = "other")]
        category: String,
    },
    List {
        cask_id: String,
    },
    Get {
        id: String,
    },
    Categorize {
        id: String,
        #[arg(long)]
        category: String,
    },
}
#[derive(Debug, Subcommand)]
enum CategoryAction {
    List { cask_id: String },
    Create { cask_id: String, name: String },
}
#[derive(Debug, Subcommand)]
enum StorageAction {
    Summary { cask_id: String },
}

#[derive(Debug, Subcommand)]
enum OperationAction {
    List {
        #[arg(long)]
        cask_id: Option<String>,
    },
    Get {
        id: String,
    },
    Recover {
        id: String,
    },
    Preserve {
        id: String,
    },
    Cancel {
        id: String,
    },
}

#[derive(Debug, Subcommand)]
enum RecoveryAction {
    Status,
    Inspect { id: String },
    RetryManifests,
}

#[derive(Debug, Subcommand)]
enum DiagnosticsAction {
    Bundle { output_path: String },
}

#[expect(
    clippy::too_many_lines,
    reason = "top-level CLI command routing stays in one match"
)]
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
        Command::Resource {
            action:
                ResourceAction::Add {
                    cask_id,
                    source_path,
                    mode,
                    category,
                },
        } => {
            let progress = |done: u64, total: u64| {
                if total == 0 {
                    eprint!("\rInspected: {done} entries");
                } else {
                    eprint!("\rProcessed: {done}/{total} bytes");
                }
            };
            let result = service
                .add_resource(
                    &AddResource {
                        version: CONTRACT_VERSION,
                        cask_id,
                        source_path,
                        mode: mode.into(),
                        category_key: category,
                    },
                    &correlation_id,
                    &progress,
                )
                .map(|resource| render_resource_added(&resource, mode, cli.output));
            eprintln!();
            result
        }
        Command::Resource {
            action: ResourceAction::List { cask_id },
        } => service
            .list_resources(
                &ListResources {
                    version: CONTRACT_VERSION,
                    cask_id,
                },
                &correlation_id,
            )
            .map(|resources| render_resource_list(&resources, cli.output)),
        Command::Resource {
            action: ResourceAction::Get { id },
        } => service
            .get_resource(
                &GetResource {
                    version: CONTRACT_VERSION,
                    id,
                },
                &correlation_id,
            )
            .map(|resource| render_resource_detail(&resource, cli.output)),
        Command::Resource {
            action: ResourceAction::Categorize { id, category },
        } => service
            .categorize_resource(
                &CategorizeResource {
                    version: CONTRACT_VERSION,
                    resource_id: id,
                    category_key: category,
                },
                &correlation_id,
            )
            .map(|resource| render_resource_detail(&resource, cli.output)),
        Command::Category {
            action: CategoryAction::List { cask_id },
        } => service
            .list_categories(
                &ListCategories {
                    version: CONTRACT_VERSION,
                    cask_id,
                },
                &correlation_id,
            )
            .map(|categories| render_category_list(&categories, cli.output)),
        Command::Category {
            action: CategoryAction::Create { cask_id, name },
        } => service
            .create_category(
                &CreateCategory {
                    version: CONTRACT_VERSION,
                    cask_id,
                    name,
                },
                &correlation_id,
            )
            .map(|category| render_category_created(&category, cli.output)),
        Command::Storage {
            action: StorageAction::Summary { cask_id },
        } => service
            .storage_summary(
                &GetStorageSummary {
                    version: CONTRACT_VERSION,
                    cask_id,
                },
                &correlation_id,
            )
            .map(|summary| render_storage_summary(&summary, cli.output)),
        Command::Operation {
            action: OperationAction::List { cask_id },
        } => service
            .list_operations(
                &ListOperations {
                    version: CONTRACT_VERSION,
                    cask_id,
                },
                &correlation_id,
            )
            .map(|items| render_operations(&items, cli.output)),
        Command::Operation {
            action: OperationAction::Get { id },
        } => service
            .get_operation(
                &GetOperation {
                    version: CONTRACT_VERSION,
                    id,
                },
                &correlation_id,
            )
            .map(|item| render_operation(&item, cli.output)),
        Command::Operation {
            action: OperationAction::Recover { id },
        } => service
            .recover_operation(
                &RecoverOperation {
                    version: CONTRACT_VERSION,
                    id,
                },
                &correlation_id,
            )
            .map(|item| render_operation(&item, cli.output)),
        Command::Operation {
            action: OperationAction::Preserve { id },
        } => service
            .preserve_operation(
                &PreserveOperation {
                    version: CONTRACT_VERSION,
                    id,
                },
                &correlation_id,
            )
            .map(|item| render_operation(&item, cli.output)),
        Command::Operation {
            action: OperationAction::Cancel { id },
        } => service
            .cancel_operation(
                &CancelOperation {
                    version: CONTRACT_VERSION,
                    id,
                },
                &correlation_id,
            )
            .map(|item| render_operation(&item, cli.output)),
        Command::Recovery {
            action: RecoveryAction::Status,
        } => service
            .recovery_status(&correlation_id)
            .map(|status| render_recovery_status(&status, cli.output)),
        Command::Recovery {
            action: RecoveryAction::Inspect { id },
        } => service
            .inspect_recovery(
                &GetOperation {
                    version: CONTRACT_VERSION,
                    id,
                },
                &correlation_id,
            )
            .map(|item| render_recovery_inspection(&item, cli.output)),
        Command::Recovery {
            action: RecoveryAction::RetryManifests,
        } => service
            .retry_manifest_projections(&correlation_id)
            .map(|count| match cli.output {
                Output::Table => format!("Regenerated {count} manifests.\n"),
                Output::Json => format!("{{\"completed\":{count}}}\n"),
            }),
        Command::Diagnostics {
            action: DiagnosticsAction::Bundle { output_path },
        } => service
            .create_diagnostics_bundle(
                &CreateDiagnosticsBundle {
                    version: CONTRACT_VERSION,
                    output_path: output_path.clone(),
                },
                &correlation_id,
            )
            .map(|()| match cli.output {
                Output::Table => format!("Wrote diagnostics to {output_path}\n"),
                Output::Json => format!("{}\n", serde_json::json!({"outputPath":output_path})),
            }),
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

fn render_operations(items: &[OperationView], output: Output) -> String {
    match output {
        Output::Json => format!(
            "{}\n",
            serde_json::to_string(items).expect("Operation DTO is serializable")
        ),
        Output::Table => {
            let mut result = String::new();
            for item in items {
                writeln!(result, "{} {} {}", item.id, item.state, item.step).expect("String write");
            }
            result
        }
    }
}

fn render_operation(item: &OperationView, output: Output) -> String {
    match output {
        Output::Json => format!(
            "{}\n",
            serde_json::to_string(item).expect("Operation DTO is serializable")
        ),
        Output::Table => format!(
            "{}\nState: {}\nStep: {}\nProgress: {}{}\n",
            item.id,
            item.state,
            item.step,
            item.progress_completed,
            item.progress_total
                .map_or_else(String::new, |total| format!("/{total}"))
        ),
    }
}

fn render_recovery_status(status: &RecoveryStatus, output: Output) -> String {
    match output {
        Output::Json => format!(
            "{}\n",
            serde_json::to_string(status).expect("Recovery DTO is serializable")
        ),
        Output::Table => {
            let mut result = String::new();
            for item in &status.operations {
                writeln!(result, "Operation {} needs recovery", item.id).expect("String write");
            }
            for id in &status.pending_manifest_casks {
                writeln!(result, "Cask {id} has a pending manifest").expect("String write");
            }
            if result.is_empty() {
                result.push_str("No recovery required.\n");
            }
            result
        }
    }
}

fn render_recovery_inspection(item: &RecoveryInspection, output: Output) -> String {
    match output {
        Output::Json => format!(
            "{}\n",
            serde_json::to_string(item).expect("Recovery DTO is serializable")
        ),
        Output::Table => format!(
            "Operation: {}\nMode: {}\nSource: {} ({})\nStaging: {} ({})\nManaged destination: {} ({})\n",
            item.operation.id,
            item.mode.to_ascii_lowercase(),
            item.source_path,
            item.source_status.to_ascii_lowercase(),
            item.staging_path,
            item.staging_status.to_ascii_lowercase(),
            item.final_path,
            item.final_status.to_ascii_lowercase()
        ),
    }
}

fn render_resource_added(resource: &ResourceView, mode: Mode, output: Output) -> String {
    match output {
        Output::Table => format!(
            "{}: {}\n",
            match mode {
                Mode::Copy => "Copied",
                Mode::Move => "Moved",
                Mode::Reference => "Referenced",
            },
            resource.name
        ),
        Output::Json => format!(
            "{}\n",
            serde_json::to_string(resource).expect("Resource DTO is serializable")
        ),
    }
}
fn render_resource_list(items: &[ResourceView], output: Output) -> String {
    match output {
        Output::Table => {
            let mut result = String::new();
            for item in items {
                writeln!(
                    result,
                    "{} ({})",
                    item.name,
                    item.management.to_ascii_lowercase()
                )
                .expect("String write");
            }
            result
        }
        Output::Json => format!(
            "{}\n",
            serde_json::to_string(items).expect("Resource DTO is serializable")
        ),
    }
}
fn render_resource_detail(item: &ResourceView, output: Output) -> String {
    match output {
        Output::Table => format!(
            "{}\nManagement: {}\nLocation: {}\nAvailability: {}\nCategory: {}\nSize: {}\n",
            item.name,
            item.management.to_ascii_lowercase(),
            item.path,
            item.availability.to_ascii_lowercase(),
            item.category_key,
            item.size_bytes.map_or_else(
                || "unknown".into(),
                |n| format!("{n} bytes ({})", item.size_measurement.to_ascii_lowercase())
            )
        ),
        Output::Json => format!(
            "{}\n",
            serde_json::to_string(item).expect("Resource DTO is serializable")
        ),
    }
}
fn render_category_list(items: &[CategoryView], output: Output) -> String {
    match output {
        Output::Table => {
            let mut result = String::new();
            for item in items {
                writeln!(result, "{} ({})", item.name, item.key).expect("String write");
            }
            result
        }
        Output::Json => format!(
            "{}\n",
            serde_json::to_string(items).expect("Category DTO is serializable")
        ),
    }
}
fn render_category_created(item: &CategoryView, output: Output) -> String {
    match output {
        Output::Table => format!("Created category: {} ({})\n", item.name, item.key),
        Output::Json => format!(
            "{}\n",
            serde_json::to_string(item).expect("Category DTO is serializable")
        ),
    }
}
fn render_storage_summary(item: &StorageSummary, output: Output) -> String {
    match output {
        Output::Table => format!(
            "Managed logical: {} bytes\nReferenced logical: {} bytes\nPhysical: unknown\nReclaimable: {}–{} bytes\nUnknown entries: {}\n",
            item.managed_logical_bytes,
            item.referenced_logical_bytes,
            item.reclaimable_min_bytes,
            item.reclaimable_max_bytes,
            item.unknown_entries
        ),
        Output::Json => format!(
            "{}\n",
            serde_json::to_string(item).expect("Summary DTO is serializable")
        ),
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
