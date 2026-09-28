//! Versioned application transport contracts.
use serde::{Deserialize, Serialize};

pub const CRATE_BOUNDARY: &str = "contracts";
pub const CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCask {
    pub version: u32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCask {
    pub version: u32,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListCasks {
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaskView {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub state: String,
    pub managed_storage_root: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub revision: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub version: u32,
    pub code: String,
    pub message: String,
    pub correlation_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AddMode {
    Copy,
    Move,
    Reference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddResource {
    pub version: u32,
    pub cask_id: String,
    pub source_path: String,
    pub mode: AddMode,
    pub category_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResources {
    pub version: u32,
    pub cask_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetResource {
    pub version: u32,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceView {
    pub version: u32,
    pub id: String,
    pub cask_id: String,
    pub name: String,
    pub path: String,
    pub kind: String,
    pub management: String,
    pub containment: String,
    pub availability: String,
    pub category_key: String,
    pub size_bytes: Option<u64>,
    pub size_measurement: String,
    pub unknown_entries: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectPortableZip {
    pub version: u32,
    pub archive_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableZipPreview {
    pub version: u32,
    pub archive_sha256: String,
    pub file_count: u32,
    pub expanded_bytes: u64,
    pub executable_paths: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPortableZip {
    pub version: u32,
    pub cask_id: String,
    pub archive_path: String,
    pub archive_sha256: String,
    pub application_name: String,
    pub application_version: Option<String>,
    pub executable_relative_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPortableInstallations {
    pub version: u32,
    pub cask_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPortableInstallation {
    pub version: u32,
    pub installation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchPortableInstallation {
    pub version: u32,
    pub installation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableInstallationView {
    pub version: u32,
    pub application_id: String,
    pub installation_id: String,
    pub cask_id: String,
    pub resource_id: String,
    pub application_name: String,
    pub application_version: Option<String>,
    pub provider: String,
    pub provenance: String,
    pub relationship: String,
    pub state: String,
    pub root_path: String,
    pub executable_relative_path: String,
    pub source_sha256: String,
    pub can_launch: bool,
    pub can_plan_removal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchInstalledApplications {
    pub version: u32,
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshInstalledApplications {
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssociateInstalledApplication {
    pub version: u32,
    pub cask_id: String,
    pub installation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisassociateInstalledApplication {
    pub version: u32,
    pub cask_id: String,
    pub installation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAssociatedApplications {
    pub version: u32,
    pub cask_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedApplicationView {
    pub version: u32,
    pub application_id: String,
    pub installation_id: String,
    pub name: String,
    pub publisher: Option<String>,
    pub display_version: Option<String>,
    pub scope: String,
    pub source_key: String,
    pub install_location: Option<String>,
    pub state: String,
    pub ambiguous_name: bool,
    pub associated_cask_id: Option<String>,
    pub provenance: String,
    pub removal_policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WingetCapabilities {
    pub version: u32,
    pub search: bool,
    pub install: bool,
    pub uninstall: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchWinget {
    pub version: u32,
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WingetSearchResult {
    pub version: u32,
    pub sanitized_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewWingetPackage {
    pub version: u32,
    pub package_id: String,
    pub package_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WingetPackageReview {
    pub version: u32,
    pub package_id: String,
    pub package_version: String,
    pub source: String,
    pub details: String,
    pub review_hash: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallWingetPackage {
    pub version: u32,
    pub cask_id: String,
    pub package_id: String,
    pub package_version: String,
    pub review_hash: String,
    pub accept_package_agreements: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListWingetInstallations {
    pub version: u32,
    pub cask_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WingetInstallationView {
    pub version: u32,
    pub id: String,
    pub cask_id: String,
    pub package_id: String,
    pub package_version: String,
    pub registry_installation_id: String,
    pub state: String,
    pub provenance: String,
    pub can_plan_uninstall: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WingetOperationView {
    pub version: u32,
    pub id: String,
    pub cask_id: String,
    pub action: String,
    pub package_id: String,
    pub state: String,
    pub step: String,
    pub error_code: Option<String>,
    pub cancellation_requested: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetWingetOperation {
    pub version: u32,
    pub id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelWingetOperation {
    pub version: u32,
    pub id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseWingetOperation {
    pub version: u32,
    pub id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWingetUninstallPlan {
    pub version: u32,
    pub installation_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveWingetUninstallPlan {
    pub version: u32,
    pub plan_id: String,
    pub plan_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteWingetUninstallPlan {
    pub version: u32,
    pub plan_id: String,
    pub plan_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WingetUninstallPlanView {
    pub version: u32,
    pub id: String,
    pub installation_id: String,
    pub package_id: String,
    pub package_version: String,
    pub state: String,
    pub plan_hash: String,
    pub expires_at_ms: i64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectInstallerArtifact {
    pub version: u32,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallerInspection {
    pub version: u32,
    pub path: String,
    pub format: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub signature_status: String,
    pub signature_detail: Option<String>,
    pub signer: Option<String>,
    pub product_name: Option<String>,
    pub publisher: Option<String>,
    pub product_version: Option<String>,
    pub product_code: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterInstallerArtifact {
    pub version: u32,
    pub cask_id: String,
    pub path: String,
    pub expected_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListInstallerArtifacts {
    pub version: u32,
    pub cask_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallerArtifactView {
    pub version: u32,
    pub id: String,
    pub cask_id: String,
    pub resource_id: String,
    pub source_path: String,
    pub inspection: InstallerInspection,
    pub registered_at_ms: i64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchInstallerArtifact {
    pub version: u32,
    pub artifact_id: String,
    pub expected_sha256: String,
    pub accept_execution_risk: bool,
    pub allow_elevation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallerCandidateView {
    pub installation_id: String,
    pub evidence_sha256: String,
    pub name: String,
    pub publisher: Option<String>,
    pub display_version: Option<String>,
    pub scope: String,
    pub confidence: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallerOperationView {
    pub version: u32,
    pub id: String,
    pub artifact_id: String,
    pub cask_id: String,
    pub state: String,
    pub exit_code: Option<i32>,
    pub error_code: Option<String>,
    pub candidates: Vec<InstallerCandidateView>,
    pub confirmed_installation_id: Option<String>,
    pub started_at_ms: i64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetInstallerOperation {
    pub version: u32,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListInstallerOperations {
    pub version: u32,
    pub cask_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmInstallerCandidate {
    pub version: u32,
    pub operation_id: String,
    pub installation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseInstallerOperation {
    pub version: u32,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelInstallerOperation {
    pub version: u32,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCategory {
    pub version: u32,
    pub cask_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListCategories {
    pub version: u32,
    pub cask_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryView {
    pub version: u32,
    pub key: String,
    pub name: String,
    pub is_builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategorizeResource {
    pub version: u32,
    pub resource_id: String,
    pub category_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetStorageSummary {
    pub version: u32,
    pub cask_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageSummary {
    pub version: u32,
    pub managed_logical_bytes: u64,
    pub referenced_logical_bytes: u64,
    pub physical_bytes: Option<u64>,
    pub reclaimable_min_bytes: u64,
    pub reclaimable_max_bytes: u64,
    pub unknown_entries: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListOperations {
    pub version: u32,
    pub cask_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetOperation {
    pub version: u32,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverOperation {
    pub version: u32,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreserveOperation {
    pub version: u32,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelOperation {
    pub version: u32,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationView {
    pub version: u32,
    pub id: String,
    pub cask_id: String,
    pub state: String,
    pub step: String,
    pub progress_completed: u64,
    pub progress_total: Option<u64>,
    pub cancellation_requested: bool,
    pub error_code: Option<String>,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryStatus {
    pub version: u32,
    pub operations: Vec<OperationView>,
    pub pending_manifest_casks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryInspection {
    pub version: u32,
    pub operation: OperationView,
    pub mode: String,
    pub source_path: String,
    pub source_status: String,
    pub staging_path: String,
    pub staging_status: String,
    pub final_path: String,
    pub final_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDiagnosticsBundle {
    pub version: u32,
    pub output_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupDatabase {
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseBackupView {
    pub version: u32,
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticEvent {
    pub operation_id: Option<String>,
    pub code: String,
    pub at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsBundle {
    pub schema_version: u32,
    pub generated_at_ms: i64,
    pub operations: Vec<OperationView>,
    pub pending_manifest_count: usize,
    pub events: Vec<DiagnosticEvent>,
    pub events_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRemovalPlan {
    pub version: u32,
    pub cask_id: String,
    pub resource_ids: Vec<String>,
    pub category_key: Option<String>,
    #[serde(default)]
    pub permanent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetRemovalPlan {
    pub version: u32,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveRemovalPlan {
    pub version: u32,
    pub id: String,
    pub plan_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteRemovalPlan {
    pub version: u32,
    pub id: String,
    pub plan_hash: String,
    #[serde(default)]
    pub confirm_permanent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseRemovalRun {
    pub version: u32,
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovalPlanItemView {
    pub id: String,
    pub resource_id: String,
    pub name: String,
    pub action: String,
    pub reason_code: String,
    pub expected_size_bytes: Option<u64>,
    pub state: String,
    pub result_code: Option<String>,
    pub recovery_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovalPlanView {
    pub version: u32,
    pub id: String,
    pub cask_id: String,
    pub scope: String,
    pub category_key: Option<String>,
    pub permanent: bool,
    pub state: String,
    pub plan_hash: String,
    pub run_id: Option<String>,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
    pub items: Vec<RemovalPlanItemView>,
}

/// Generated frontend declarations for the versioned transport surface.
pub const TYPESCRIPT: &str = r"// Generated by cargo xtask generate-contracts. Do not edit.
export const CONTRACT_VERSION = 1;
export type CreateCask = { version: number; name: string };
export type GetCask = { version: number; id: string };
export type ListCasks = { version: number };
export type CaskView = { version: number; id: string; name: string; state: string; managedStorageRoot: string; createdAtMs: number; updatedAtMs: number; revision: number };
export type ApiError = { version: number; code: string; message: string; correlationId: string };
export type AddMode = 'copy' | 'move' | 'reference';
export type AddResource = { version: number; caskId: string; sourcePath: string; mode: AddMode; categoryKey: string };
export type ListResources = { version: number; caskId: string };
export type GetResource = { version: number; id: string };
export type ResourceView = { version: number; id: string; caskId: string; name: string; path: string; kind: string; management: string; containment: string; availability: string; categoryKey: string; sizeBytes: number | null; sizeMeasurement: string; unknownEntries: number };
export type InspectPortableZip = { version: number; archivePath: string };
export type PortableZipPreview = { version: number; archiveSha256: string; fileCount: number; expandedBytes: number; executablePaths: string[]; warnings: string[] };
export type InstallPortableZip = { version: number; caskId: string; archivePath: string; archiveSha256: string; applicationName: string; applicationVersion: string | null; executableRelativePath: string };
export type ListPortableInstallations = { version: number; caskId: string };
export type GetPortableInstallation = { version: number; installationId: string };
export type LaunchPortableInstallation = { version: number; installationId: string };
export type PortableInstallationView = { version: number; applicationId: string; installationId: string; caskId: string; resourceId: string; applicationName: string; applicationVersion: string | null; provider: string; provenance: string; relationship: string; state: string; rootPath: string; executableRelativePath: string; sourceSha256: string; canLaunch: boolean; canPlanRemoval: boolean };
export type SearchInstalledApplications = { version: number; query: string };
export type RefreshInstalledApplications = { version: number };
export type AssociateInstalledApplication = { version: number; caskId: string; installationId: string };
export type DisassociateInstalledApplication = { version: number; caskId: string; installationId: string };
export type ListAssociatedApplications = { version: number; caskId: string };
export type DetectedApplicationView = { version: number; applicationId: string; installationId: string; name: string; publisher: string | null; displayVersion: string | null; scope: string; sourceKey: string; installLocation: string | null; state: string; ambiguousName: boolean; associatedCaskId: string | null; provenance: string; removalPolicy: string };
export type WingetCapabilities = { version: number; search: boolean; install: boolean; uninstall: boolean; reason: string | null };
export type SearchWinget = { version: number; query: string };
export type WingetSearchResult = { version: number; sanitizedText: string };
export type ReviewWingetPackage = { version: number; packageId: string; packageVersion: string };
export type WingetPackageReview = { version: number; packageId: string; packageVersion: string; source: string; details: string; reviewHash: string; warnings: string[] };
export type InstallWingetPackage = { version: number; caskId: string; packageId: string; packageVersion: string; reviewHash: string; acceptPackageAgreements: boolean };
export type ListWingetInstallations = { version: number; caskId: string };
export type WingetInstallationView = { version: number; id: string; caskId: string; packageId: string; packageVersion: string; registryInstallationId: string; state: string; provenance: string; canPlanUninstall: boolean; warnings: string[] };
export type WingetOperationView = { version: number; id: string; caskId: string; action: string; packageId: string; state: string; step: string; errorCode: string | null; cancellationRequested: boolean };
export type GetWingetOperation = { version: number; id: string };
export type CancelWingetOperation = { version: number; id: string };
export type CloseWingetOperation = { version: number; id: string };
export type CreateWingetUninstallPlan = { version: number; installationId: string };
export type ApproveWingetUninstallPlan = { version: number; planId: string; planHash: string };
export type ExecuteWingetUninstallPlan = { version: number; planId: string; planHash: string };
export type WingetUninstallPlanView = { version: number; id: string; installationId: string; packageId: string; packageVersion: string; state: string; planHash: string; expiresAtMs: number; warnings: string[] };
export type InspectInstallerArtifact = { version: number; path: string };
export type InstallerInspection = { version: number; path: string; format: string; sha256: string; sizeBytes: number; signatureStatus: string; signatureDetail: string | null; signer: string | null; productName: string | null; publisher: string | null; productVersion: string | null; productCode: string | null; warnings: string[] };
export type RegisterInstallerArtifact = { version: number; caskId: string; path: string; expectedSha256: string };
export type ListInstallerArtifacts = { version: number; caskId: string };
export type InstallerArtifactView = { version: number; id: string; caskId: string; resourceId: string; sourcePath: string; inspection: InstallerInspection; registeredAtMs: number; warnings: string[] };
export type LaunchInstallerArtifact = { version: number; artifactId: string; expectedSha256: string; acceptExecutionRisk: boolean; allowElevation: boolean };
export type InstallerCandidateView = { installationId: string; evidenceSha256: string; name: string; publisher: string | null; displayVersion: string | null; scope: string; confidence: string; evidence: string[] };
export type InstallerOperationView = { version: number; id: string; artifactId: string; caskId: string; state: string; exitCode: number | null; errorCode: string | null; candidates: InstallerCandidateView[]; confirmedInstallationId: string | null; startedAtMs: number; warnings: string[] };
export type GetInstallerOperation = { version: number; id: string };
export type ListInstallerOperations = { version: number; caskId: string };
export type ConfirmInstallerCandidate = { version: number; operationId: string; installationId: string };
export type CloseInstallerOperation = { version: number; id: string };
export type CancelInstallerOperation = { version: number; id: string };
export type CreateCategory = { version: number; caskId: string; name: string };
export type ListCategories = { version: number; caskId: string };
export type CategoryView = { version: number; key: string; name: string; isBuiltin: boolean };
export type CategorizeResource = { version: number; resourceId: string; categoryKey: string };
export type GetStorageSummary = { version: number; caskId: string };
export type StorageSummary = { version: number; managedLogicalBytes: number; referencedLogicalBytes: number; physicalBytes: number | null; reclaimableMinBytes: number; reclaimableMaxBytes: number; unknownEntries: number };
export type ListOperations = { version: number; caskId: string | null };
export type GetOperation = { version: number; id: string };
export type RecoverOperation = { version: number; id: string };
export type PreserveOperation = { version: number; id: string };
export type CancelOperation = { version: number; id: string };
export type OperationView = { version: number; id: string; caskId: string; state: string; step: string; progressCompleted: number; progressTotal: number | null; cancellationRequested: boolean; errorCode: string | null; updatedAtMs: number };
export type RecoveryStatus = { version: number; operations: OperationView[]; pendingManifestCasks: string[] };
export type RecoveryInspection = { version: number; operation: OperationView; mode: string; sourcePath: string; sourceStatus: string; stagingPath: string; stagingStatus: string; finalPath: string; finalStatus: string };
export type CreateDiagnosticsBundle = { version: number; outputPath: string };
export type BackupDatabase = { version: number };
export type DatabaseBackupView = { version: number; path: string; sizeBytes: number };
export type DiagnosticEvent = { operationId: string | null; code: string; atMs: number };
export type DiagnosticsBundle = { schemaVersion: number; generatedAtMs: number; operations: OperationView[]; pendingManifestCount: number; events: DiagnosticEvent[]; eventsTruncated: boolean };
export type CreateRemovalPlan = { version: number; caskId: string; resourceIds: string[]; categoryKey: string | null; permanent: boolean };
export type GetRemovalPlan = { version: number; id: string };
export type ApproveRemovalPlan = { version: number; id: string; planHash: string };
export type ExecuteRemovalPlan = { version: number; id: string; planHash: string; confirmPermanent: boolean };
export type CloseRemovalRun = { version: number; runId: string };
export type RemovalPlanItemView = { id: string; resourceId: string; name: string; action: string; reasonCode: string; expectedSizeBytes: number | null; state: string; resultCode: string | null; recoveryPath: string | null };
export type RemovalPlanView = { version: number; id: string; caskId: string; scope: string; categoryKey: string | null; permanent: boolean; state: string; planHash: string; runId: string | null; createdAtMs: number; expiresAtMs: number; items: RemovalPlanItemView[] };
";
