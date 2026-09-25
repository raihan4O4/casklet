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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetRemovalPlan {
    pub version: u32,
    pub id: String,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovalPlanView {
    pub version: u32,
    pub id: String,
    pub cask_id: String,
    pub scope: String,
    pub category_key: Option<String>,
    pub state: String,
    pub plan_hash: String,
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
export type DiagnosticEvent = { operationId: string | null; code: string; atMs: number };
export type DiagnosticsBundle = { schemaVersion: number; generatedAtMs: number; operations: OperationView[]; pendingManifestCount: number; events: DiagnosticEvent[]; eventsTruncated: boolean };
export type CreateRemovalPlan = { version: number; caskId: string; resourceIds: string[]; categoryKey: string | null };
export type GetRemovalPlan = { version: number; id: string };
export type RemovalPlanItemView = { id: string; resourceId: string; name: string; action: string; reasonCode: string; expectedSizeBytes: number | null };
export type RemovalPlanView = { version: number; id: string; caskId: string; scope: string; categoryKey: string | null; state: string; planHash: string; createdAtMs: number; expiresAtMs: number; items: RemovalPlanItemView[] };
";
