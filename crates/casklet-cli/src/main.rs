use casklet_contracts::{
    AddMode, AddResource, ApiError, AssociateInstalledApplication, CONTRACT_VERSION,
    CancelOperation, CaskView, CategorizeResource, CategoryView, CreateCask, CreateCategory,
    CreateDiagnosticsBundle, CreateRemovalPlan, DetectedApplicationView,
    DisassociateInstalledApplication, GetCask, GetOperation, GetPortableInstallation,
    GetRemovalPlan, GetResource, GetStorageSummary, InspectPortableZip, InstallPortableZip,
    LaunchPortableInstallation, ListAssociatedApplications, ListCasks, ListCategories,
    ListOperations, ListPortableInstallations, ListResources, OperationView,
    PortableInstallationView, PreserveOperation, RecoverOperation, RecoveryInspection,
    RecoveryStatus, RefreshInstalledApplications, RemovalPlanView, ResourceView,
    SearchInstalledApplications, StorageSummary,
};
#[cfg(feature = "reviewed-removal")]
use casklet_contracts::{ApproveRemovalPlan, CloseRemovalRun, ExecuteRemovalPlan};
use casklet_infrastructure::inventory::{
    ExportProfile, describe_manifests, discover_manifests, preview_manifest, write_export,
};
use clap::{Parser, Subcommand, ValueEnum};
use std::fmt::Write;
use std::path::Path;
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(name = "casklet", about = "Manage your Casks")]
struct Cli {
    #[arg(long, value_enum, default_value_t = Output::Table, global = true)]
    output: Output,
    #[cfg(feature = "reviewed-removal")]
    #[arg(long, global = true)]
    no_input: bool,
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
    Export {
        cask_id: String,
        output_path: String,
        #[arg(long, value_enum, default_value_t = ExportFormat::Json)]
        format: ExportFormat,
        #[arg(long)]
        machine: bool,
    },
    Manifest {
        #[command(subcommand)]
        action: ManifestAction,
    },
    Portable {
        #[command(subcommand)]
        action: PortableAction,
    },
    Installed {
        #[command(subcommand)]
        action: InstalledAction,
    },
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
    Plan {
        #[command(subcommand)]
        action: PlanAction,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExportFormat {
    Json,
    Markdown,
}

#[derive(Debug, Subcommand)]
enum ManifestAction {
    Preview { path: String },
    Recover { path: String },
    Scan,
    RecoverAll,
}

#[derive(Debug, Subcommand)]
enum PortableAction {
    Inspect {
        archive_path: String,
    },
    Install {
        cask_id: String,
        archive_path: String,
        #[arg(long)]
        hash: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        executable: String,
        #[arg(long)]
        app_version: Option<String>,
    },
    List {
        cask_id: String,
    },
    Get {
        installation_id: String,
    },
    Launch {
        installation_id: String,
    },
}

#[derive(Debug, Subcommand)]
enum InstalledAction {
    Search {
        query: String,
    },
    Refresh,
    Associate {
        cask_id: String,
        installation_id: String,
    },
    Disassociate {
        cask_id: String,
        installation_id: String,
    },
    List {
        cask_id: String,
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

#[derive(Debug, Subcommand)]
enum PlanAction {
    Create {
        cask_id: String,
        #[arg(long, conflicts_with = "category")]
        resource: Vec<String>,
        #[arg(long)]
        category: Option<String>,
        #[arg(long)]
        permanent: bool,
    },
    Get {
        id: String,
    },
    #[cfg(feature = "reviewed-removal")]
    Approve {
        id: String,
        #[arg(long)]
        hash: String,
    },
    #[cfg(feature = "reviewed-removal")]
    Execute {
        id: String,
        #[arg(long)]
        hash: String,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        confirm_permanent: bool,
    },
    #[cfg(feature = "reviewed-removal")]
    #[command(alias = "close-preserved")]
    CloseUncertain {
        run_id: String,
        #[arg(long)]
        yes: bool,
    },
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
        Command::Installed {
            action: InstalledAction::Search { query },
        } => service
            .search_installed_applications(
                &SearchInstalledApplications {
                    version: CONTRACT_VERSION,
                    query,
                },
                &correlation_id,
            )
            .map(|items| render_detected(&items, cli.output)),
        Command::Installed {
            action: InstalledAction::Refresh,
        } => service
            .refresh_installed_applications(
                &RefreshInstalledApplications {
                    version: CONTRACT_VERSION,
                },
                &correlation_id,
            )
            .map(|count| format!("Reconciled {count} detected installations\n")),
        Command::Installed {
            action:
                InstalledAction::Associate {
                    cask_id,
                    installation_id,
                },
        } => service
            .associate_installed_application(
                &AssociateInstalledApplication {
                    version: CONTRACT_VERSION,
                    cask_id,
                    installation_id,
                },
                &correlation_id,
            )
            .map(|item| render_detected(&[item], cli.output)),
        Command::Installed {
            action:
                InstalledAction::Disassociate {
                    cask_id,
                    installation_id,
                },
        } => service
            .disassociate_installed_application(
                &DisassociateInstalledApplication {
                    version: CONTRACT_VERSION,
                    cask_id,
                    installation_id,
                },
                &correlation_id,
            )
            .map(|()| "Association removed; installation unchanged\n".into()),
        Command::Installed {
            action: InstalledAction::List { cask_id },
        } => service
            .list_associated_applications(
                &ListAssociatedApplications {
                    version: CONTRACT_VERSION,
                    cask_id,
                },
                &correlation_id,
            )
            .map(|items| render_detected(&items, cli.output)),
        Command::Portable {
            action: PortableAction::Inspect { archive_path },
        } => service
            .inspect_portable_zip(
                &InspectPortableZip {
                    version: CONTRACT_VERSION,
                    archive_path,
                },
                &correlation_id,
            )
            .map(|preview| match cli.output {
                Output::Json => format!("{}\n", serde_json::to_string(&preview).unwrap()),
                Output::Table => format!(
                    "ZIP SHA-256: {}\n{} files; {} expanded bytes\nExecutables:\n{}\n",
                    preview.archive_sha256,
                    preview.file_count,
                    preview.expanded_bytes,
                    preview.executable_paths.join("\n")
                ),
            }),
        Command::Portable {
            action:
                PortableAction::Install {
                    cask_id,
                    archive_path,
                    hash,
                    name,
                    executable,
                    app_version,
                },
        } => service
            .install_portable_zip(
                &InstallPortableZip {
                    version: CONTRACT_VERSION,
                    cask_id,
                    archive_path,
                    archive_sha256: hash,
                    application_name: name,
                    application_version: app_version,
                    executable_relative_path: executable,
                },
                &correlation_id,
                &|_, _| {},
                &|_| {},
            )
            .map(|item| render_portable(&item, cli.output)),
        Command::Portable {
            action: PortableAction::List { cask_id },
        } => service
            .list_portable_installations(
                &ListPortableInstallations {
                    version: CONTRACT_VERSION,
                    cask_id,
                },
                &correlation_id,
            )
            .map(|items| match cli.output {
                Output::Json => format!("{}\n", serde_json::to_string(&items).unwrap()),
                Output::Table => {
                    use std::fmt::Write as _;
                    items.iter().fold(String::new(), |mut output, item| {
                        let _ = writeln!(
                            output,
                            "{} {} ({})",
                            item.installation_id, item.application_name, item.state
                        );
                        output
                    })
                }
            }),
        Command::Portable {
            action: PortableAction::Get { installation_id },
        } => service
            .get_portable_installation(
                &GetPortableInstallation {
                    version: CONTRACT_VERSION,
                    installation_id,
                },
                &correlation_id,
            )
            .map(|item| render_portable(&item, cli.output)),
        Command::Portable {
            action: PortableAction::Launch { installation_id },
        } => service
            .launch_portable_installation(
                &LaunchPortableInstallation {
                    version: CONTRACT_VERSION,
                    installation_id,
                },
                &correlation_id,
            )
            .map(|item| {
                format!(
                    "Started {} ({})\n",
                    item.application_name, item.installation_id
                )
            }),
        Command::Export {
            cask_id,
            output_path,
            format,
            machine,
        } => {
            let profile = if machine {
                ExportProfile::Machine
            } else {
                ExportProfile::Portable
            };
            let content = match format {
                ExportFormat::Json => service
                    .repository
                    .export_inventory(&cask_id, profile)
                    .and_then(|value| serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())),
                ExportFormat::Markdown => service
                    .repository
                    .export_report(&cask_id, profile)
                    .map(String::into_bytes),
            };
            content
                .and_then(|bytes| write_export(Path::new(&output_path), &bytes))
                .map(|()| format!("Wrote {output_path}\n"))
                .map_err(|message| ApiError {
                    version: CONTRACT_VERSION,
                    code: "EXPORT_FAILED".into(),
                    message,
                    correlation_id: correlation_id.clone(),
                })
        }
        Command::Manifest {
            action: ManifestAction::Preview { path },
        } => preview_manifest(Path::new(&path))
            .map(|preview| {
                format!(
                    "{}: {} resources ({} present, {} missing, {} unverified); {}\n",
                    preview.cask_name,
                    preview.resource_count,
                    preview.observed_present,
                    preview.observed_missing,
                    preview.unverified,
                    preview.warnings.join(" ")
                )
            })
            .map_err(|message| ApiError {
                version: CONTRACT_VERSION,
                code: "MANIFEST_INVALID".into(),
                message,
                correlation_id: correlation_id.clone(),
            }),
        Command::Manifest {
            action: ManifestAction::Recover { path },
        } => service
            .repository
            .recover_manifest(&service.storage, Path::new(&path))
            .map(|preview| {
                format!(
                    "Recovered {} as unverified, detach-only metadata ({} resources).\n",
                    preview.cask_name, preview.resource_count
                )
            })
            .map_err(|message| ApiError {
                version: CONTRACT_VERSION,
                code: "RECOVERY_FAILED".into(),
                message,
                correlation_id: correlation_id.clone(),
            }),
        Command::Manifest {
            action: ManifestAction::Scan,
        } => discover_manifests(&service.storage)
            .map(|items| describe_manifests(&items))
            .map_err(|message| ApiError {
                version: CONTRACT_VERSION,
                code: "MANIFEST_INVALID".into(),
                message,
                correlation_id: correlation_id.clone(),
            }),
        Command::Manifest {
            action: ManifestAction::RecoverAll,
        } => service
            .repository
            .recover_discovered(&service.storage)
            .map(|items| format!("Recovered {} Casks as unverified metadata.\n", items.len()))
            .map_err(|message| ApiError {
                version: CONTRACT_VERSION,
                code: "RECOVERY_FAILED".into(),
                message,
                correlation_id: correlation_id.clone(),
            }),
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
        Command::Plan {
            action:
                PlanAction::Create {
                    cask_id,
                    resource,
                    category,
                    permanent,
                },
        } => service
            .create_removal_plan(
                &CreateRemovalPlan {
                    version: CONTRACT_VERSION,
                    cask_id,
                    resource_ids: resource,
                    category_key: category,
                    permanent,
                },
                &correlation_id,
            )
            .map(|plan| render_plan(&plan, cli.output)),
        Command::Plan {
            action: PlanAction::Get { id },
        } => service
            .get_removal_plan(
                &GetRemovalPlan {
                    version: CONTRACT_VERSION,
                    id,
                },
                &correlation_id,
            )
            .map(|plan| render_plan(&plan, cli.output)),
        #[cfg(feature = "reviewed-removal")]
        Command::Plan {
            action: PlanAction::Approve { id, hash },
        } => service
            .approve_removal_plan(
                &ApproveRemovalPlan {
                    version: CONTRACT_VERSION,
                    id,
                    plan_hash: hash,
                },
                &correlation_id,
            )
            .map(|plan| render_plan(&plan, cli.output)),
        #[cfg(feature = "reviewed-removal")]
        Command::Plan {
            action:
                PlanAction::Execute {
                    id,
                    hash,
                    yes,
                    confirm_permanent,
                },
        } => confirm_removal(&id, yes, cli.no_input)
            .and_then(|()| {
                service.execute_removal_plan(
                    &ExecuteRemovalPlan {
                        version: CONTRACT_VERSION,
                        id,
                        plan_hash: hash,
                        confirm_permanent,
                    },
                    &correlation_id,
                )
            })
            .map(|plan| render_plan(&plan, cli.output)),
        #[cfg(feature = "reviewed-removal")]
        Command::Plan {
            action: PlanAction::CloseUncertain { run_id, yes },
        } => confirm_removal(&run_id, yes, cli.no_input)
            .and_then(|()| {
                service.close_uncertain_removal(
                    &CloseRemovalRun {
                        version: CONTRACT_VERSION,
                        run_id,
                    },
                    &correlation_id,
                )
            })
            .map(|plan| render_plan(&plan, cli.output)),
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

#[cfg(feature = "reviewed-removal")]
fn confirm_removal(id: &str, yes: bool, no_input: bool) -> Result<(), ApiError> {
    if yes {
        return Ok(());
    }
    if no_input {
        return Err(ApiError {
            version: CONTRACT_VERSION,
            code: "CONFIRMATION_REQUIRED".into(),
            message: "Pass --yes to run without input".into(),
            correlation_id: "cli".into(),
        });
    }
    eprintln!("Review the plan first. Type {id} to confirm this cleanup action:");
    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .map_err(|_| ApiError {
            version: CONTRACT_VERSION,
            code: "INPUT_FAILED".into(),
            message: "Could not read confirmation".into(),
            correlation_id: "cli".into(),
        })?;
    if answer.trim() != id {
        return Err(ApiError {
            version: CONTRACT_VERSION,
            code: "CANCELLED".into(),
            message: "Confirmation did not match".into(),
            correlation_id: "cli".into(),
        });
    }
    Ok(())
}

fn render_plan(plan: &RemovalPlanView, output: Output) -> String {
    match output {
        Output::Json => format!(
            "{}\n",
            serde_json::to_string(plan).expect("Plan DTO is serializable")
        ),
        Output::Table => {
            let mut result = format!(
                "Plan {} ({}; {}; expires {})\nHash: {}\n",
                plan.id,
                plan.state.to_ascii_lowercase(),
                if plan.permanent {
                    "permanent files"
                } else {
                    "Recycle Bin"
                },
                plan.expires_at_ms,
                plan.plan_hash,
            );
            if let Some(run_id) = &plan.run_id {
                writeln!(result, "Run: {run_id}").expect("String write");
            }
            for item in &plan.items {
                writeln!(
                    result,
                    "{}: {} ({}) [{}{}]",
                    item.name,
                    item.action.to_ascii_lowercase(),
                    item.reason_code.to_ascii_lowercase(),
                    item.state.to_ascii_lowercase(),
                    item.result_code
                        .as_ref()
                        .map_or_else(String::new, |code| format!("; {code}")),
                )
                .expect("String write");
                if let Some(path) = &item.recovery_path {
                    writeln!(result, "  Possible recovery location: {path}").expect("String write");
                }
            }
            result
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

fn render_detected(items: &[DetectedApplicationView], output: Output) -> String {
    match output {
        Output::Json => format!("{}\n", serde_json::to_string(items).unwrap()),
        Output::Table => items.iter().fold(String::new(), |mut text, item| {
            let _ = writeln!(
                text,
                "{} | {} | {} | {} | {} | {}",
                item.installation_id,
                item.name,
                item.display_version.as_deref().unwrap_or(""),
                item.scope,
                item.state,
                item.source_key
            );
            text
        }),
    }
}

fn render_portable(item: &PortableInstallationView, output: Output) -> String {
    match output {
        Output::Json => format!(
            "{}\n",
            serde_json::to_string(item).expect("Portable DTO is serializable")
        ),
        Output::Table => format!(
            "{}\nInstallation: {}\nState: {}\nExecutable: {}\nManaged resource: {}\n",
            item.application_name,
            item.installation_id,
            item.state,
            item.executable_relative_path,
            item.resource_id
        ),
    }
}
