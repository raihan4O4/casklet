<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { onMount, tick } from "svelte";
  import type {
    AddMode,
    ApiError,
    CaskView,
    CategoryView,
    ResourceView,
    StorageSummary,
    RecoveryStatus,
    RecoveryInspection,
    OperationView,
    RemovalPlanView,
    PortableZipPreview,
    PortableInstallationView,
    DetectedApplicationView,
    WingetCapabilities,
    WingetPackageReview,
    WingetInstallationView,
    WingetUninstallPlanView,
    WingetOperationView,
    WingetSearchResult,
    InstallerInspection,
    InstallerArtifactView,
    InstallerOperationView,
    DatabaseBackupView,
  } from "../contracts/generated/casklet";
  import { CONTRACT_VERSION } from "../contracts/generated/casklet";
  import PopupMenu from "./PopupMenu.svelte";
  import { notify } from "./notifications";
  import Icon from "./Icon.svelte";
  import { caskColor, activityDay, bytes, relativeTime, label } from "./format";
  let {
    caskId = null,
    mode = "detail",
    onReady = () => {},
  }: {
    caskId?: string | null;
    mode?: "detail" | "activity" | "settings";
    onReady?: () => void;
  } = $props();
  let loading = $state(true);
  let adding = $state(false);
  let applicationWorkflow:
    | "portable"
    | "winget"
    | "installer"
    | "existing"
    | null = $state(null);
  let editName = $state("");
  let editDescription = $state("");
  let editArchived = $state(false);
  let metadataBusy = $state(false);
  let selected: CaskView | null = $state(null);
  let detailTab:
    | "overview"
    | "files"
    | "apps"
    | "activity"
    | "settings"
    | "cleanup" = $state("overview");
  let error = $state("");
  let busy = $state(false);
  let backupBusy = $state(false);
  let resources: ResourceView[] = $state([]);
  let categories: CategoryView[] = $state([]);
  let summary: StorageSummary | null = $state(null);
  let pendingPath = $state("");
  let pendingMode: AddMode | "" = $state("");
  let pendingCategory = $state("other");
  let progress = $state("");
  let categoryName = $state("");
  let selectedResource: ResourceView | null = $state(null);
  let recovery: RecoveryStatus | null = $state(null);
  let recoveryInspection: RecoveryInspection | null = $state(null);
  let activeOperation = $state("");
  let operations: OperationView[] = $state([]);
  const recentOperations = $derived(
    [...operations].sort((a, b) => b.updatedAtMs - a.updatedAtMs),
  );
  $effect(() => {
    if (error) {
      notify(error, "error", "workspace-error");
      error = "";
    }
  });
  $effect(() => {
    if (progress) {
      notify(
        progress,
        /entries inspected|bytes processed/.test(progress) ? "info" : "success",
        "workspace-status",
      );
      progress = "";
    }
  });
  let removalPlan: RemovalPlanView | null = $state(null);
  let removalCategory = $state("");
  let removalEnabled = $state(false);
  let removalConfirmation = $state("");
  let permanentPlan = $state(false);
  let permanentConfirmation = $state(false);
  let removalBusy = $state(false);
  let portablePreview: PortableZipPreview | null = $state(null);
  let portablePath = $state("");
  let portableExe = $state("");
  let portableName = $state("");
  let portableVersion = $state("");
  let portableItems: PortableInstallationView[] = $state([]);
  let portableBusy = $state(false);
  let installedQuery = $state("");
  let detectedApps: DetectedApplicationView[] = $state([]);
  let associatedApps: DetectedApplicationView[] = $state([]);
  let installedBusy = $state(false);
  let installedSearched = $state(false);
  let referencePicker = $state(false);
  let wingetCapabilities: WingetCapabilities | null = $state(null);
  let wingetQuery = $state("");
  let wingetSearchText = $state("");
  let wingetPackageId = $state("");
  let wingetVersion = $state("");
  let wingetReview: WingetPackageReview | null = $state(null);
  let wingetAccept = $state(false);
  let wingetItems: WingetInstallationView[] = $state([]);
  let wingetPlan: WingetUninstallPlanView | null = $state(null);
  let wingetOperationId = $state("");
  let wingetOperation: WingetOperationView | null = $state(null);
  let wingetBusy = $state(false);
  let wingetConfirmation = $state("");
  let installerPath = $state("");
  let installerPreview: InstallerInspection | null = $state(null);
  let installerItems: InstallerArtifactView[] = $state([]);
  let installerOperations: InstallerOperationView[] = $state([]);
  let installerOperation: InstallerOperationView | null = $state(null);
  let installerOperationId = $state("");
  let installerSelected: InstallerArtifactView | null = $state(null);
  let installerAccept = $state(false);
  let installerElevate = $state(false);
  let installerBusy = $state(false);
  let fileResources = $derived(
    resources.filter(
      (resource) =>
        !portableItems.some((item) => item.resourceId === resource.id),
    ),
  );
  let appCount = $derived(
    new Set([
      ...portableItems.map((item) => item.installationId),
      ...associatedApps.map((item) => item.installationId),
      ...wingetItems.map((item) => item.registryInstallationId),
    ]).size,
  );
  let portableBytes = $derived(
    resources
      .filter(
        (resource) =>
          portableItems.some((item) => item.resourceId === resource.id) &&
          resource.management === "MANAGED" &&
          resource.availability === "PRESENT",
      )
      .reduce((sum, item) => sum + (item.sizeBytes ?? 0), 0),
  );
  let managedFileBytes = $derived.by(() =>
    Math.max(0, (summary?.managedLogicalBytes ?? 0) - portableBytes),
  );
  let totalKnownBytes = $derived.by(
    () =>
      (summary?.managedLogicalBytes ?? 0) +
      (summary?.referencedLogicalBytes ?? 0),
  );
  async function inspectInstaller() {
    try {
      const path = await open({
        multiple: false,
        filters: [{ name: "Windows installer", extensions: ["msi", "exe"] }],
      });
      if (typeof path !== "string") return;
      installerBusy = true;
      installerPreview = await invoke<InstallerInspection>(
        "inspect_installer_artifact",
        { request: { version: CONTRACT_VERSION, path } },
      );
      installerPath = path;
      error = "";
    } catch (cause) {
      error = message(cause);
    } finally {
      installerBusy = false;
    }
  }
  async function registerInstaller() {
    if (!selected || !installerPreview) return;
    installerBusy = true;
    try {
      await invoke<InstallerArtifactView>("register_installer_artifact", {
        request: {
          version: CONTRACT_VERSION,
          caskId: selected.id,
          path: installerPath,
          expectedSha256: installerPreview.sha256,
        },
      });
      installerPreview = null;
      await refreshInstallers();
      await refreshResources();
      progress = "Installer copy registered without launching it.";
      error = "";
    } catch (cause) {
      error = message(cause);
    } finally {
      installerBusy = false;
    }
  }
  async function refreshInstallers() {
    if (!selected) return;
    const caskId = selected.id;
    installerItems = await invoke<InstallerArtifactView[]>(
      "list_installer_artifacts",
      { request: { version: CONTRACT_VERSION, caskId } },
    );
    installerOperations = await invoke<InstallerOperationView[]>(
      "list_installer_operations",
      { request: { version: CONTRACT_VERSION, caskId } },
    );
  }
  async function launchInstaller() {
    if (!installerSelected || !installerAccept) return;
    installerBusy = true;
    try {
      installerOperation = await invoke<InstallerOperationView>(
        "launch_installer_artifact",
        {
          request: {
            version: CONTRACT_VERSION,
            artifactId: installerSelected.id,
            expectedSha256: installerSelected.inspection.sha256,
            acceptExecutionRisk: true,
            allowElevation: installerElevate,
          },
        },
      );
      installerOperationId = installerOperation.id;
      await refreshInstallers();
      progress =
        "Installer launch finished. Review observed candidates; none is owned automatically.";
      error = "";
    } catch (cause) {
      error = message(cause);
      await refreshInstallers().catch(() => {});
    } finally {
      installerBusy = false;
      installerAccept = false;
      if (installerOperationId) void refreshInstallerOperation();
    }
  }
  async function refreshInstallerOperation() {
    if (installerOperationId) {
      const result = await invoke<InstallerOperationView>(
        "get_installer_operation",
        { request: { version: CONTRACT_VERSION, id: installerOperationId } },
      );
      if (result.caskId === selected?.id) installerOperation = result;
    }
  }
  async function confirmInstallerCandidate(id: string) {
    if (!installerOperation) return;
    try {
      installerOperation = await invoke<InstallerOperationView>(
        "confirm_installer_candidate",
        {
          request: {
            version: CONTRACT_VERSION,
            operationId: installerOperation.id,
            installationId: id,
          },
        },
      );
      await refreshAssociated();
      await refreshInstallers();
      progress =
        "Candidate associated as a reference. Casklet did not claim installation ownership.";
      error = "";
    } catch (cause) {
      error = message(cause);
    }
  }
  async function cancelInstaller() {
    if (!installerOperationId) return;
    try {
      await invoke("cancel_installer_operation", {
        request: { version: CONTRACT_VERSION, id: installerOperationId },
      });
      await refreshInstallerOperation();
    } catch (cause) {
      error = message(cause);
    }
  }
  async function closeInstaller() {
    if (!installerOperation) return;
    try {
      await invoke("close_installer_operation", {
        request: { version: CONTRACT_VERSION, id: installerOperation.id },
      });
      await refreshInstallerOperation();
      await refreshInstallers();
      progress = "Installer outcome preserved for manual inspection.";
    } catch (cause) {
      error = message(cause);
    }
  }
  function resetInstallerReview() {
    installerPreview = null;
    installerSelected = null;
    installerAccept = false;
    installerElevate = false;
    installerOperation = null;
    installerOperationId = "";
  }
  async function searchWinget() {
    wingetBusy = true;
    try {
      const result = await invoke<WingetSearchResult>("search_winget", {
        request: { version: CONTRACT_VERSION, query: wingetQuery },
      });
      wingetSearchText = result.sanitizedText;
      error = "";
    } catch (cause) {
      error = message(cause);
    } finally {
      wingetBusy = false;
    }
  }
  async function reviewWinget() {
    wingetBusy = true;
    wingetReview = null;
    wingetAccept = false;
    try {
      wingetReview = await invoke<WingetPackageReview>(
        "review_winget_package",
        {
          request: {
            version: CONTRACT_VERSION,
            packageId: wingetPackageId,
            packageVersion: wingetVersion,
          },
        },
      );
      error = "";
    } catch (cause) {
      error = message(cause);
    } finally {
      wingetBusy = false;
    }
  }
  async function installWinget() {
    if (!selected || !wingetReview || !wingetAccept) return;
    wingetBusy = true;
    try {
      await invoke<WingetInstallationView>("install_winget_package", {
        request: {
          version: CONTRACT_VERSION,
          caskId: selected.id,
          packageId: wingetReview.packageId,
          packageVersion: wingetReview.packageVersion,
          reviewHash: wingetReview.reviewHash,
          acceptPackageAgreements: true,
        },
      });
      wingetReview = null;
      wingetAccept = false;
      progress = "WinGet installation verified and recorded.";
      await refreshWinget();
      error = "";
    } catch (cause) {
      error = message(cause);
    } finally {
      wingetBusy = false;
      if (wingetOperationId) void refreshWingetOperation();
    }
  }
  async function refreshWinget() {
    if (selected)
      wingetItems = await invoke<WingetInstallationView[]>(
        "list_winget_installations",
        { request: { version: CONTRACT_VERSION, caskId: selected.id } },
      );
  }
  async function refreshWingetOperation() {
    if (wingetOperationId)
      wingetOperation = await invoke<WingetOperationView>(
        "get_winget_operation",
        { request: { version: CONTRACT_VERSION, id: wingetOperationId } },
      );
  }
  async function cancelWinget() {
    if (!wingetOperationId) return;
    try {
      await invoke("cancel_winget_operation", {
        request: { version: CONTRACT_VERSION, id: wingetOperationId },
      });
      await refreshWingetOperation();
    } catch (cause) {
      error = message(cause);
    }
  }
  async function closeWinget() {
    if (!wingetOperationId) return;
    try {
      await invoke("close_winget_operation", {
        request: { version: CONTRACT_VERSION, id: wingetOperationId },
      });
      await refreshWingetOperation();
      progress = "Uncertain WinGet work kept for manual inspection.";
    } catch (cause) {
      error = message(cause);
    }
  }
  async function planWingetUninstall(item: WingetInstallationView) {
    wingetBusy = true;
    try {
      wingetPlan = await invoke<WingetUninstallPlanView>(
        "create_winget_uninstall_plan",
        { request: { version: CONTRACT_VERSION, installationId: item.id } },
      );
      wingetConfirmation = "";
      error = "";
    } catch (cause) {
      error = message(cause);
    } finally {
      wingetBusy = false;
    }
  }
  async function approveWingetUninstall() {
    if (!wingetPlan) return;
    try {
      wingetPlan = await invoke<WingetUninstallPlanView>(
        "approve_winget_uninstall_plan",
        {
          request: {
            version: CONTRACT_VERSION,
            planId: wingetPlan.id,
            planHash: wingetPlan.planHash,
          },
        },
      );
      error = "";
    } catch (cause) {
      error = message(cause);
    }
  }
  async function executeWingetUninstall() {
    if (!wingetPlan || wingetConfirmation !== wingetPlan.id) return;
    wingetBusy = true;
    try {
      wingetPlan = await invoke<WingetUninstallPlanView>(
        "execute_winget_uninstall_plan",
        {
          request: {
            version: CONTRACT_VERSION,
            planId: wingetPlan.id,
            planHash: wingetPlan.planHash,
          },
        },
      );
      await refreshWinget();
      progress = "WinGet uninstall verified.";
      error = "";
    } catch (cause) {
      error = message(cause);
    } finally {
      wingetBusy = false;
      if (wingetOperationId) void refreshWingetOperation();
    }
  }
  async function searchInstalled() {
    installedBusy = true;
    try {
      detectedApps = await invoke<DetectedApplicationView[]>(
        "search_installed_applications",
        { request: { version: CONTRACT_VERSION, query: installedQuery } },
      );
      installedSearched = true;
      error = "";
    } catch (cause) {
      error = message(cause);
    } finally {
      installedBusy = false;
    }
  }
  async function refreshInstalled() {
    installedBusy = true;
    try {
      const count = await invoke<number>("refresh_installed_applications", {
        request: { version: CONTRACT_VERSION },
      });
      progress = `Reconciled ${count} detected installations.`;
      await searchInstalled();
      await refreshAssociated();
      error = "";
    } catch (cause) {
      error = message(cause);
    } finally {
      installedBusy = false;
    }
  }
  async function refreshAssociated() {
    if (!selected) return;
    associatedApps = await invoke<DetectedApplicationView[]>(
      "list_associated_applications",
      { request: { version: CONTRACT_VERSION, caskId: selected.id } },
    );
  }
  async function associateDetected(item: DetectedApplicationView) {
    if (!selected) return;
    installedBusy = true;
    try {
      await invoke("associate_installed_application", {
        request: {
          version: CONTRACT_VERSION,
          caskId: selected.id,
          installationId: item.installationId,
        },
      });
      await refreshAssociated();
      progress =
        "Associated existing installation. Casklet did not install or own it.";
      error = "";
    } catch (cause) {
      error = message(cause);
    } finally {
      installedBusy = false;
    }
  }
  async function disassociateDetected(item: DetectedApplicationView) {
    if (!selected) return;
    installedBusy = true;
    try {
      await invoke("disassociate_installed_application", {
        request: {
          version: CONTRACT_VERSION,
          caskId: selected.id,
          installationId: item.installationId,
        },
      });
      await refreshAssociated();
      progress = "Association removed. The installation is unchanged.";
      error = "";
    } catch (cause) {
      error = message(cause);
    } finally {
      installedBusy = false;
    }
  }
  async function inspectPortableArchive() {
    try {
      const path = await open({
        multiple: false,
        filters: [{ name: "ZIP archive", extensions: ["zip"] }],
      });
      if (typeof path !== "string") return;
      portableBusy = true;
      portablePreview = await invoke<PortableZipPreview>(
        "inspect_portable_zip",
        { request: { version: CONTRACT_VERSION, archivePath: path } },
      );
      portablePath = path;
      portableExe =
        portablePreview.executablePaths.length === 1
          ? portablePreview.executablePaths[0]
          : "";
      portableName =
        path
          .split(/[\\/]/)
          .pop()
          ?.replace(/\.zip$/i, "") ?? "";
      error = "";
    } catch (cause) {
      portablePreview = null;
      error = message(cause);
    } finally {
      portableBusy = false;
    }
  }
  async function installPortableArchive() {
    if (!selected || !portablePreview || !portableExe || !portableName.trim())
      return;
    portableBusy = true;
    try {
      await invoke<PortableInstallationView>("install_portable_zip", {
        request: {
          version: CONTRACT_VERSION,
          caskId: selected.id,
          archivePath: portablePath,
          archiveSha256: portablePreview.archiveSha256,
          applicationName: portableName,
          applicationVersion: portableVersion.trim() || null,
          executableRelativePath: portableExe,
        },
      });
      portablePreview = null;
      portablePath = "";
      portableExe = "";
      portableVersion = "";
      progress =
        "Portable application installed. Launch only if you trust its source.";
      await refreshResources();
      await refreshOperations();
      error = "";
    } catch (cause) {
      error = message(cause);
      await refreshRecovery();
    } finally {
      portableBusy = false;
    }
  }
  async function launchPortable(item: PortableInstallationView) {
    try {
      await invoke("launch_portable_installation", {
        request: {
          version: CONTRACT_VERSION,
          installationId: item.installationId,
        },
      });
      progress = `Started ${item.applicationName}.`;
      error = "";
    } catch (cause) {
      error = message(cause);
    }
  }
  async function previewPortableRemoval(item: PortableInstallationView) {
    await previewRemoval([item.resourceId]);
  }
  let recoveryManifestPath = $state("");
  let recoveryManifestPreview = $state("");
  let discoveredManifests = $state("");
  async function scanRecoveryManifests() {
    try {
      discoveredManifests = await invoke<string>("scan_recovery_manifests");
      if (!discoveredManifests)
        discoveredManifests = "No local manifests found.";
      error = "";
    } catch (cause) {
      discoveredManifests = "";
      error = message(cause);
    }
  }
  async function recoverDiscoveredManifests() {
    try {
      progress = await invoke<string>("recover_discovered_manifests");
      discoveredManifests = "";
      await refresh();
    } catch (cause) {
      error = message(cause);
    }
  }
  async function exportCask(format: "json" | "markdown", machine = false) {
    if (!selected) return;
    try {
      const outputPath = await save({
        defaultPath: `${selected.name}-inventory.${format === "json" ? "json" : "md"}`,
      });
      if (!outputPath) return;
      await invoke("export_cask", {
        caskId: selected.id,
        outputPath,
        format,
        machine,
      });
      progress = "Inventory saved. Review it before sharing.";
      error = "";
    } catch (cause) {
      error = message(cause);
    }
  }
  async function chooseRecoveryManifest() {
    try {
      const path = await open({
        multiple: false,
        filters: [{ name: "Casklet manifest", extensions: ["json"] }],
      });
      if (typeof path !== "string") return;
      recoveryManifestPath = path;
      recoveryManifestPreview = await invoke<string>(
        "preview_recovery_manifest",
        { path },
      );
      error = "";
    } catch (cause) {
      recoveryManifestPreview = "";
      error = message(cause);
    }
  }
  async function recoverFromManifest() {
    if (!recoveryManifestPath || !recoveryManifestPreview) return;
    try {
      progress = await invoke<string>("recover_manifest", {
        path: recoveryManifestPath,
      });
      recoveryManifestPath = "";
      recoveryManifestPreview = "";
      await refresh();
    } catch (cause) {
      error = message(cause);
    }
  }
  async function previewRemoval(resourceIds: string[] = []) {
    if (!selected) return;
    try {
      removalPlan = await invoke<RemovalPlanView>("create_removal_plan", {
        request: {
          version: CONTRACT_VERSION,
          caskId: selected.id,
          resourceIds,
          categoryKey: resourceIds.length ? null : removalCategory || null,
          permanent: permanentPlan,
        },
      });
      detailTab = "cleanup";
      permanentConfirmation = false;
      error = "";
    } catch (cause) {
      error = message(cause);
    }
  }
  function previewSelectedResource() {
    if (selectedResource) void previewRemoval([selectedResource.id]);
  }
  async function approveRemoval() {
    if (!removalPlan || !removalEnabled) return;
    removalBusy = true;
    try {
      removalPlan = await invoke<RemovalPlanView>("approve_removal_plan", {
        request: {
          version: CONTRACT_VERSION,
          id: removalPlan.id,
          planHash: removalPlan.planHash,
        },
      });
      error = "";
    } catch (cause) {
      error = message(cause);
    } finally {
      removalBusy = false;
    }
  }
  async function executeRemoval() {
    if (
      !removalPlan ||
      !removalEnabled ||
      removalConfirmation !== removalPlan.id ||
      (removalPlan.permanent && !permanentConfirmation)
    )
      return;
    removalBusy = true;
    try {
      removalPlan = await invoke<RemovalPlanView>("execute_removal_plan", {
        request: {
          version: CONTRACT_VERSION,
          id: removalPlan.id,
          planHash: removalPlan.planHash,
          confirmPermanent: removalPlan.permanent && permanentConfirmation,
        },
      });
      removalConfirmation = "";
      await refreshResources();
      if (selected)
        selected = await invoke<CaskView>("get_cask", {
          request: { version: CONTRACT_VERSION, id: selected.id },
        });
      error = "";
    } catch (cause) {
      error = message(cause);
    } finally {
      removalBusy = false;
    }
  }
  async function closeUncertainRemoval() {
    if (!removalPlan?.runId || removalConfirmation !== removalPlan.runId)
      return;
    removalBusy = true;
    try {
      removalPlan = await invoke<RemovalPlanView>("close_uncertain_removal", {
        request: {
          version: CONTRACT_VERSION,
          runId: removalPlan.runId,
        },
      });
      removalConfirmation = "";
      if (selected)
        selected = await invoke<CaskView>("get_cask", {
          request: { version: CONTRACT_VERSION, id: selected.id },
        });
      error = "";
    } catch (cause) {
      error = message(cause);
    } finally {
      removalBusy = false;
    }
  }
  function message(cause: unknown): string {
    const api = cause as Partial<ApiError>;
    return api?.message ?? String(cause);
  }
  async function refresh() {
    try {
      await refreshRecovery();
      await refreshOperations();
      error = "";
    } catch (cause) {
      error = message(cause);
    }
  }
  async function refreshRecovery() {
    recovery = await invoke<RecoveryStatus>("recovery_status");
    if (selected)
      recovery = {
        ...recovery,
        operations: recovery.operations.filter(
          (item) => item.caskId === selected?.id,
        ),
        pendingManifestCasks: recovery.pendingManifestCasks.filter(
          (id) => id === selected?.id,
        ),
      };
  }
  async function refreshOperations() {
    operations = await invoke<OperationView[]>("list_operations", {
      request: { version: CONTRACT_VERSION, caskId: selected?.id ?? null },
    });
  }
  async function recoverOperation(id: string) {
    try {
      await invoke("recover_operation", {
        request: { version: CONTRACT_VERSION, id },
      });
      await refreshRecovery();
      await refreshResources();
    } catch (cause) {
      error = message(cause);
      await refreshRecovery();
    }
  }
  async function retryManifests() {
    try {
      await invoke("retry_manifests");
      await refreshRecovery();
    } catch (cause) {
      error = message(cause);
    }
  }
  async function inspectRecovery(id: string) {
    try {
      recoveryInspection = await invoke<RecoveryInspection>(
        "inspect_recovery",
        { request: { version: CONTRACT_VERSION, id } },
      );
    } catch (cause) {
      error = message(cause);
    }
  }
  async function preserveOperation(id: string) {
    try {
      await invoke("preserve_operation", {
        request: { version: CONTRACT_VERSION, id },
      });
      await refreshRecovery();
    } catch (cause) {
      error = message(cause);
    }
  }
  async function cancelOperation(id: string) {
    try {
      await invoke("cancel_operation", {
        request: { version: CONTRACT_VERSION, id },
      });
      progress =
        "Cancellation requested. Casklet will stop at a safe boundary.";
      await refreshOperations();
    } catch (cause) {
      error = message(cause);
    }
  }
  async function saveDiagnostics() {
    try {
      const outputPath = await save({
        defaultPath: "casklet-diagnostics.json",
      });
      if (outputPath) {
        await invoke("create_diagnostics_bundle", {
          request: { version: CONTRACT_VERSION, outputPath },
        });
        progress = "Diagnostic bundle saved.";
      }
    } catch (cause) {
      error = message(cause);
    }
  }
  async function backupDatabase() {
    backupBusy = true;
    try {
      const backup = await invoke<DatabaseBackupView>("backup_database", {
        request: { version: CONTRACT_VERSION },
      });
      progress = `Database metadata backup saved to ${backup.path} (${backup.sizeBytes} bytes). Managed Cask files are not included.`;
      error = "";
    } catch (cause) {
      error = `Database backup failed: ${message(cause)}. Check free disk space; the original database remains in place.`;
    } finally {
      backupBusy = false;
    }
  }
  async function inspect(id: string) {
    try {
      selected = await invoke<CaskView>("get_cask", {
        request: { version: CONTRACT_VERSION, id },
      });
      detailTab = "overview";
      resetInstallerReview();
      pendingPath = "";
      pendingMode = "";
      selectedResource = null;
      removalPlan = null;
      error = "";
      await tick();
      document.getElementById("cask-detail-heading")?.focus();
      await refreshResources();
      editName = selected.name;
      editDescription = selected.description ?? "";
      editArchived = selected.state === "ARCHIVED";
    } catch (cause) {
      error = message(cause);
    }
  }
  async function refreshResources() {
    if (!selected) return;
    const caskId = selected.id;
    selected = await invoke<CaskView>("get_cask", {
      request: { version: CONTRACT_VERSION, id: caskId },
    });
    resources = await invoke<ResourceView[]>("list_resources", {
      request: { version: CONTRACT_VERSION, caskId },
    });
    portableItems = await invoke<PortableInstallationView[]>(
      "list_portable_installations",
      { request: { version: CONTRACT_VERSION, caskId } },
    );
    await refreshWinget().catch(() => {
      wingetItems = [];
    });
    await refreshInstallers().catch(() => {
      installerItems = [];
      installerOperations = [];
    });
    await refreshAssociated().catch(() => {
      associatedApps = [];
    });
    categories = await invoke<CategoryView[]>("list_categories", {
      request: { version: CONTRACT_VERSION, caskId },
    });
    summary = await invoke<StorageSummary>("storage_summary", {
      request: { version: CONTRACT_VERSION, caskId },
    });
  }
  async function pick(directory: boolean, reference = false) {
    try {
      const picked = await open({ multiple: false, directory });
      if (typeof picked === "string") {
        pendingPath = picked;
        pendingMode = reference ? "reference" : "";
        referencePicker = false;
      }
    } catch (cause) {
      error = message(cause);
    }
  }
  async function addResource() {
    if (!selected || !pendingPath || !pendingMode) return;
    busy = true;
    error = "";
    try {
      selectedResource = await invoke<ResourceView>("add_resource", {
        request: {
          version: CONTRACT_VERSION,
          caskId: selected.id,
          sourcePath: pendingPath,
          mode: pendingMode,
          categoryKey: pendingCategory,
        },
      });
      pendingPath = "";
      pendingMode = "";
      adding = false;
      progress = "Resource added.";
      await refreshResources();
    } catch (cause) {
      error = message(cause);
    } finally {
      await refreshRecovery().catch(() => {});
      await refreshOperations().catch(() => {});
      activeOperation = "";
      busy = false;
    }
  }
  async function createCategory() {
    if (!selected || !categoryName.trim()) return;
    try {
      const category = await invoke<CategoryView>("create_category", {
        request: {
          version: CONTRACT_VERSION,
          caskId: selected.id,
          name: categoryName,
        },
      });
      categories = [...categories, category];
      progress = "Category created.";
      categoryName = "";
      pendingCategory = category.key;
    } catch (cause) {
      error = message(cause);
    }
  }
  async function recategorize(resource: ResourceView, categoryKey: string) {
    try {
      selectedResource = await invoke<ResourceView>("categorize_resource", {
        request: {
          version: CONTRACT_VERSION,
          resourceId: resource.id,
          categoryKey,
        },
      });
      await refreshResources();
      progress = "Changes saved.";
    } catch (cause) {
      error = message(cause);
    }
  }
  onMount(() => {
    void initialize();
    void invoke<boolean>("removal_capability")
      .then((enabled) => {
        removalEnabled = enabled;
      })
      .catch(() => {
        removalEnabled = false;
      });
    void invoke<WingetCapabilities>("winget_capabilities")
      .then((value) => {
        wingetCapabilities = value;
      })
      .catch(() => {
        wingetCapabilities = null;
      });
    let disposed = false;
    let unlistenDrop: (() => void) | undefined;
    let unlistenProgress: (() => void) | undefined;
    let unlistenPortableProgress: (() => void) | undefined;
    let unlistenStarted: (() => void) | undefined;
    let unlistenWinget: (() => void) | undefined;
    let unlistenInstaller: (() => void) | undefined;
    const poll = window.setInterval(() => {
      void refreshRecovery().catch(() => {});
      void refreshOperations().catch(() => {});
      void refreshWingetOperation().catch(() => {});
      void refreshInstallerOperation().catch(() => {});
    }, 3000);
    void getCurrentWindow()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop" && event.payload.paths.length > 0) {
          if (event.payload.paths.length > 1) {
            error = "Drop one item at a time.";
            return;
          }
          if (!selected) return;
          detailTab = "files";
          adding = true;
          pendingPath = event.payload.paths[0];
          pendingMode = "";
        }
      })
      .then((unlisten) => {
        if (disposed) unlisten();
        else unlistenDrop = unlisten;
      })
      .catch((cause) => {
        error = message(cause);
      });
    void listen<[number, number]>("resource.progress", (event) => {
      const [done, total] = event.payload;
      progress =
        total === 0
          ? `${done} entries inspected`
          : `${done} of ${total} bytes processed`;
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenProgress = unlisten;
    });
    void listen<string>("operation.started", (event) => {
      activeOperation = event.payload;
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenStarted = unlisten;
    });
    void listen<[number, number]>("portable.progress", (event) => {
      const [done, total] = event.payload;
      progress =
        total === 0
          ? `${done} entries inspected`
          : `${done} of ${total} bytes processed`;
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenPortableProgress = unlisten;
    });
    void listen<string>("winget.operation.started", (event) => {
      wingetOperationId = event.payload;
      void refreshWingetOperation();
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenWinget = unlisten;
    });
    void listen<string>("installer.operation.started", (event) => {
      installerOperationId = event.payload;
      void refreshInstallerOperation();
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenInstaller = unlisten;
    });
    return () => {
      disposed = true;
      window.clearInterval(poll);
      unlistenDrop?.();
      unlistenProgress?.();
      unlistenPortableProgress?.();
      unlistenStarted?.();
      unlistenWinget?.();
      unlistenInstaller?.();
    };
  });
  async function initialize() {
    try {
      if (caskId) await inspect(caskId);
      await refresh();
    } catch (cause) {
      error = message(cause);
    } finally {
      loading = false;
      await tick();
      onReady();
    }
  }
  async function openStorage() {
    if (!selected) return;
    try {
      await invoke("open_cask_storage", {
        request: { version: CONTRACT_VERSION, id: selected.id },
      });
    } catch (cause) {
      error = message(cause);
    }
  }
  async function saveMetadata() {
    if (!selected) return;
    metadataBusy = true;
    try {
      selected = await invoke<CaskView>("update_cask", {
        request: {
          version: CONTRACT_VERSION,
          id: selected.id,
          name: editName,
          description: editDescription,
          archived: editArchived,
          expectedRevision: selected.revision,
        },
      });
      progress = "Cask settings saved.";
      error = "";
    } catch (cause) {
      error = message(cause);
    } finally {
      metadataBusy = false;
    }
  }
  async function closeWorkspace() {
    try {
      await getCurrentWindow().close();
    } catch (cause) {
      error = message(cause);
    }
  }
</script>

<div class:detail-window={mode === "detail"} class="workspace">
  {#if mode === "detail"}
    <header class="workspace-header">
      <div class="workspace-topline">
        <button
          class="icon-button"
          type="button"
          aria-label="Close Cask workspace"
          title="Close Cask workspace"
          onclick={() => void closeWorkspace()}><Icon name="back" /></button
        >
        <div class="workspace-menu">
          <PopupMenu id="workspace-actions" label="Cask actions">
            <button
              role="menuitem"
              type="button"
              onclick={() => {
                detailTab = "settings";
              }}>Cask settings</button
            >
            <button
              role="menuitem"
              type="button"
              onclick={() => void exportCask("markdown")}>Export report</button
            >
            <button
              role="menuitem"
              type="button"
              onclick={() => void openStorage()}>Open storage folder</button
            >
          </PopupMenu>
        </div>
      </div>
      <div class="workspace-title">
        <span
          class="status-dot"
          style:background={selected?.state === "ACTIVE"
            ? caskColor(selected.id)
            : "#999"}
          title={label(selected?.state ?? "LOADING")}
        ></span>
        <h1 id="cask-detail-heading" tabindex="-1" title={selected?.name}>
          {selected?.name ?? "Opening Cask…"}
        </h1>
      </div>
      <p class="cask-description">
        {selected?.description ||
          "Files, applications and resources for this activity."}
      </p>
    </header>
    <nav class="detail-tabs" aria-label="Cask sections">
      {#each [{ key: "overview", title: "Overview" }, { key: "files", title: "Files & Folders" }, { key: "apps", title: "Applications" }, { key: "activity", title: "Activity" }, { key: "settings", title: "Settings" }] as tab (tab.key)}
        <button
          type="button"
          aria-current={detailTab === tab.key ? "page" : undefined}
          class:active={detailTab === tab.key}
          onclick={() => {
            detailTab = tab.key as typeof detailTab;
          }}>{tab.title}</button
        >
      {/each}
    </nav>
  {/if}
  <div class="workspace-content">
    {#if loading}<p class="loading" role="status">Loading workspace…</p>{/if}
    {#if selected && detailTab === "overview"}
      <section class="storage-panel" aria-label="Storage summary">
        <div class="section-heading">
          <h2><Icon name="database" size={20} /> Storage</h2>
          <strong class="storage-total">{bytes(totalKnownBytes)}</strong>
        </div>
        <div class="storage-bar" aria-label="Known logical storage breakdown">
          <span
            class="storage-files"
            style:width={`${totalKnownBytes ? (managedFileBytes / totalKnownBytes) * 100 : 0}%`}
          ></span><span
            class="storage-apps"
            style:width={`${totalKnownBytes ? (portableBytes / totalKnownBytes) * 100 : 0}%`}
          ></span><span
            class="storage-refs"
            style:width={`${totalKnownBytes ? ((summary?.referencedLogicalBytes ?? 0) / totalKnownBytes) * 100 : 0}%`}
          ></span>
        </div>
        <div class="storage-legend">
          <div>
            <span class="swatch files"></span>Managed files & folders<strong
              >{bytes(managedFileBytes)}</strong
            >
          </div>
          <div>
            <span class="swatch apps"></span>Portable applications<strong
              >{bytes(portableBytes)}</strong
            >
          </div>
          <div>
            <span class="swatch refs"></span>References<strong
              >{bytes(summary?.referencedLogicalBytes ?? 0)}</strong
            >
          </div>
        </div>
        <p class="footnote">
          Known logical size · {summary?.unknownEntries
            ? `${summary.unknownEntries} unmeasured or unavailable entries`
            : "Physical size is not measured"}. Associated application sizes are
          not included.
        </p>
      </section>
      <div class="compact-stats">
        <div>
          <Icon name="file" /><span
            >Total Items<strong>{fileResources.length + appCount}</strong></span
          >
        </div>
        <div>
          <Icon name="folder" /><span
            >Files & Folders<strong>{fileResources.length}</strong></span
          >
        </div>
        <div>
          <Icon name="package" /><span
            >Applications<strong>{appCount}</strong></span
          >
        </div>
      </div>
      <section class="information-panel" aria-label="Cask information">
        <div class="info-row">
          <Icon name="folder" /><span>Storage Location</span><span
            class="truncate path"
            title={selected.managedStorageRoot}
            >{selected.managedStorageRoot}</span
          ><button type="button" onclick={() => void openStorage()}>Open</button
          >
        </div>
        <div class="info-row">
          <Icon name="clock" /><span>Created</span><span
            >{new Date(selected.createdAtMs).toLocaleDateString(undefined, {
              day: "numeric",
              month: "short",
              year: "numeric",
            })}</span
          >
        </div>
        <div class="info-row">
          <Icon name="clock" /><span>Last Updated</span><span
            >{relativeTime(selected.updatedAtMs)}</span
          >
        </div>
        <div class="info-row">
          <Icon name="storage" /><span>Size on Disk</span><span
            >{bytes(summary?.physicalBytes)}</span
          >
        </div>
      </section>
    {/if}
    {#if mode === "activity" || (selected && detailTab === "activity")}
      <details class="activity-tools">
        <summary>Manifest recovery tools</summary>
        <div class="tool-actions">
          <button type="button" onclick={() => void chooseRecoveryManifest()}
            >Preview recovery manifest</button
          >
          <button type="button" onclick={() => void scanRecoveryManifests()}
            >Scan local recovery manifests</button
          >
          {#if discoveredManifests}
            <pre>{discoveredManifests}</pre>
            {#if discoveredManifests !== "No local manifests found."}<button
                type="button"
                onclick={() => void recoverDiscoveredManifests()}
                >Recover missing Casks as unverified metadata</button
              >{/if}
          {/if}
          {#if recoveryManifestPreview}
            <p>{recoveryManifestPreview}</p>
            <button type="button" onclick={() => void recoverFromManifest()}
              >Recover unverified metadata</button
            >
          {/if}
        </div>
      </details>
      <section id="operations" aria-label="Operation center">
        <h2>Activity</h2>
        {#if recentOperations.length === 0}<div class="empty-state compact">
            <Icon name="activity" size={24} />
            <h3>No activity yet</h3>
            <p>Resource operations and updates will appear here.</p>
          </div>{:else}<ul class="activity-list">
            {#each recentOperations as operation, index (operation.id)}
              {#if index === 0 || activityDay(operation.updatedAtMs) !== activityDay(recentOperations[index - 1].updatedAtMs)}<li
                  class="activity-day"
                >
                  {activityDay(operation.updatedAtMs)}
                </li>{/if}
              <li class="activity-entry">
                <Icon
                  name={operation.state === "COMPLETE"
                    ? "check"
                    : operation.state === "RECOVERY_REQUIRED"
                      ? "info"
                      : "activity"}
                  size={17}
                />
                <div class="activity-copy">
                  <div class="activity-title">
                    <strong>{label(operation.step)}</strong><time
                      title={new Date(operation.updatedAtMs).toLocaleString()}
                      >{new Date(operation.updatedAtMs).toLocaleTimeString(
                        undefined,
                        { hour: "numeric", minute: "2-digit" },
                      )}</time
                    >
                  </div>
                  <span class="activity-state">{label(operation.state)}</span>
                  {#if operation.state === "RUNNING"}<p
                      class="activity-progress"
                    >
                      {operation.progressCompleted}{operation.progressTotal ===
                      null
                        ? " entries processed"
                        : ` of ${operation.progressTotal} processed`}
                    </p>{/if}
                  <div class="activity-actions">
                    <details class="metadata-disclosure">
                      <summary>Details</summary>
                      <p class="mono">Operation {operation.id}</p>
                      {#if operation.errorCode}<p>
                          {label(operation.errorCode)}
                        </p>{/if}
                    </details>
                    {#if operation.state === "RUNNING"}<button
                        class="text-button"
                        type="button"
                        disabled={operation.cancellationRequested}
                        onclick={() => void cancelOperation(operation.id)}
                        >{operation.cancellationRequested
                          ? "Cancellation requested"
                          : "Cancel"}</button
                      >{/if}
                  </div>
                </div>
              </li>
            {/each}
          </ul>{/if}
      </section>
      {#if recovery && (recovery.operations.length || recovery.pendingManifestCasks.length)}
        <section id="recovery" aria-label="Recovery status">
          <h2>Recovery needed</h2>
          <p>
            Casklet keeps uncertain files for inspection. Recovery only
            completes work that was already registered and still verifies.
          </p>
          <ul>
            {#each recovery.operations as operation (operation.id)}
              <li>
                <div class="recovery-entry-heading">
                  <strong>{label(operation.step)}</strong><span
                    >{label(operation.state)}</span
                  >
                </div>
                <details class="metadata-disclosure">
                  <summary>Operation details</summary>
                  <p class="mono">{operation.id}</p>
                </details>
                <div class="recovery-actions">
                  <button
                    type="button"
                    onclick={() => void inspectRecovery(operation.id)}
                    >Inspect paths</button
                  >
                  <button
                    type="button"
                    onclick={() => void recoverOperation(operation.id)}
                    >Verify and recover</button
                  >
                  <button
                    type="button"
                    onclick={() => void preserveOperation(operation.id)}
                    >Keep files and close</button
                  >
                </div>
              </li>
            {/each}
          </ul>
          {#if recoveryInspection}<dl>
              <dt>Mode</dt>
              <dd>{recoveryInspection.mode}</dd>
              <dt>Source</dt>
              <dd>
                {recoveryInspection.sourcePath} ({recoveryInspection.sourceStatus})
              </dd>
              <dt>Staging</dt>
              <dd>
                {recoveryInspection.stagingPath} ({recoveryInspection.stagingStatus})
              </dd>
              <dt>Managed destination</dt>
              <dd>
                {recoveryInspection.finalPath} ({recoveryInspection.finalStatus})
              </dd>
            </dl>{/if}
          {#if recovery.pendingManifestCasks.length}<p>
              {recovery.pendingManifestCasks.length} Cask manifests await regeneration.
              <button type="button" onclick={() => void retryManifests()}
                >Retry manifests</button
              >
            </p>{/if}
        </section>
      {/if}
    {/if}
    {#if selected && detailTab === "apps"}
      <div class="section-heading">
        <h2>Associated applications</h2>
        <span class="count">{appCount}</span>
      </div>
      <p class="intro">
        Installations belong to applications. An association records their
        relationship to this Cask.
      </p>
      {#if appCount === 0}<div class="empty-state compact">
          <Icon name="package" size={28} />
          <h3>No applications yet</h3>
          <p>Add a portable app or associate an existing installation.</p>
        </div>{/if}
      <ul class="application-list">
        {#each portableItems as item (item.installationId)}<li>
            <Icon name="package" />
            <div class="item-text">
              <strong title={item.applicationName}
                >{item.applicationName}</strong
              ><span
                >{item.applicationVersion ?? "Version not recorded"} · Managed portable
                installation</span
              >
            </div>
            <button
              type="button"
              disabled={!item.canLaunch || portableBusy}
              onclick={() => void launchPortable(item)}>Launch</button
            ><button
              class="icon-button"
              aria-label={`Preview removal of ${item.applicationName}`}
              type="button"
              disabled={!item.canPlanRemoval}
              onclick={() => void previewPortableRemoval(item)}
              ><Icon name="trash" size={16} /></button
            >
          </li>{/each}
        {#each associatedApps as item (item.installationId)}<li>
            <Icon name="link" />
            <div class="item-text">
              <strong title={item.name}>{item.name}</strong><span
                >{item.displayVersion ?? "Version not recorded"} · Associated · {label(
                  item.state,
                )}</span
              ><small title={item.installationId}
                >Installation: {item.installationId}</small
              >
            </div>
            <button
              class="icon-button"
              type="button"
              aria-label={`Detach ${item.name}`}
              disabled={installedBusy}
              onclick={() => void disassociateDetected(item)}
              ><Icon name="close" size={16} /></button
            >
          </li>{/each}
        {#each wingetItems.filter((item) => !associatedApps.some((app) => app.installationId === item.registryInstallationId)) as item (item.id)}<li
          >
            <Icon name="package" />
            <div class="item-text">
              <strong>{item.packageId}</strong><span
                >{item.packageVersion} · WinGet · {label(item.state)}</span
              >
            </div>
            <button
              type="button"
              disabled={!item.canPlanUninstall}
              onclick={() => {
                applicationWorkflow = "winget";
                void planWingetUninstall(item);
              }}>Review</button
            >
          </li>{/each}
      </ul>
      <details class="workflow-picker">
        <summary
          ><Icon name="plus" /> Add an application <Icon
            name="down"
            size={14}
          /></summary
        >
        <div class="workflow-options">
          {#each [{ key: "existing", title: "Associate existing app", description: "Reference a Windows installation" }, { key: "portable", title: "Install portable ZIP", description: "Review and install in managed storage" }, { key: "winget", title: "Install with WinGet", description: "Search and review a package" }, { key: "installer", title: "Register MSI / EXE", description: "Inspect an installer before launch" }] as workflow (workflow.key)}<button
              type="button"
              onclick={() => {
                applicationWorkflow =
                  workflow.key as typeof applicationWorkflow;
              }}
              ><strong>{workflow.title}</strong><span
                >{workflow.description}</span
              ></button
            >{/each}
        </div>
      </details>
      {#if applicationWorkflow}<div class="section-heading workflow-heading">
          <span>Application workflow</span><button
            class="text-button"
            type="button"
            onclick={() => {
              applicationWorkflow = null;
            }}>Close</button
          >
        </div>{/if}
      {#if applicationWorkflow === "portable"}
        <section aria-label="Portable applications">
          <h3>Portable applications</h3>
          <p>
            Review a ZIP before installing it. Portable applications are not
            sandboxed; launch only software you trust.
          </p>
          <button
            type="button"
            disabled={portableBusy}
            onclick={() => void inspectPortableArchive()}
            >Inspect portable ZIP</button
          >
          {#if portablePreview}
            <div class="review">
              <p><strong>Archive:</strong> {portablePath}</p>
              <p><strong>SHA-256:</strong> {portablePreview.archiveSha256}</p>
              <p>
                {portablePreview.fileCount} files, {portablePreview.expandedBytes}
                expanded bytes
              </p>
              <label for="portable-name">Application name</label><input
                id="portable-name"
                bind:value={portableName}
                maxlength="120"
              />
              <label for="portable-version">Version (optional)</label><input
                id="portable-version"
                bind:value={portableVersion}
                maxlength="80"
              />
              <label for="portable-executable">Executable to launch</label>
              <select id="portable-executable" bind:value={portableExe}>
                <option value="">Choose executable</option>
                {#each portablePreview.executablePaths as executable (executable)}<option
                    value={executable}>{executable}</option
                  >{/each}
              </select>
              <button
                type="button"
                disabled={portableBusy || !portableExe || !portableName.trim()}
                onclick={() => void installPortableArchive()}
                >Install into this Cask</button
              >
            </div>
          {/if}
          {#if portableItems.length}
            <ul>
              {#each portableItems as item (item.installationId)}
                <li>
                  {item.applicationName}
                  {item.applicationVersion ?? ""} ({item.state}) — {item.executableRelativePath}
                  <button
                    type="button"
                    disabled={!item.canLaunch}
                    onclick={() => void launchPortable(item)}>Launch</button
                  >
                  <button
                    type="button"
                    disabled={!item.canPlanRemoval}
                    onclick={() => void previewPortableRemoval(item)}
                    >Preview removal</button
                  >
                </li>
              {/each}
            </ul>
          {/if}
        </section>
      {/if}
      {#if applicationWorkflow === "winget"}
        <section aria-label="WinGet packages">
          <h3>WinGet packages</h3>
          <p>
            WinGet may install for your user or the whole machine. Casklet
            records ownership only when the package and one new installed
            instance both verify.
          </p>
          {#if wingetCapabilities?.search}
            <label for="winget-search">Search WinGet</label><input
              id="winget-search"
              bind:value={wingetQuery}
              maxlength="120"
            />
            <button
              type="button"
              disabled={wingetBusy || !wingetQuery.trim()}
              onclick={() => void searchWinget()}>Search WinGet</button
            >
            {#if wingetSearchText}<pre>{wingetSearchText}</pre>{/if}
            <label for="winget-id">Exact package ID</label><input
              id="winget-id"
              bind:value={wingetPackageId}
              maxlength="120"
            />
            <label for="winget-version">Exact version</label><input
              id="winget-version"
              bind:value={wingetVersion}
              maxlength="80"
            />
            <button
              type="button"
              disabled={wingetBusy || !wingetPackageId || !wingetVersion}
              onclick={() => void reviewWinget()}>Review package</button
            >
            {#if wingetReview}
              <div class="review">
                <p>
                  {wingetReview.packageId}
                  {wingetReview.packageVersion} from {wingetReview.source}
                </p>
                <pre>{wingetReview.details}</pre>
                <p>{wingetReview.warnings.join(" ")}</p>
                <label
                  ><input type="checkbox" bind:checked={wingetAccept} /> I reviewed
                  this package and accept its package agreements</label
                >
                <button
                  type="button"
                  disabled={wingetBusy || !wingetAccept}
                  onclick={() => void installWinget()}
                  >Install for this Cask</button
                >
              </div>
            {/if}
          {:else}<p>
              {wingetCapabilities?.reason ??
                "WinGet capability is being checked."}
            </p>{/if}
          {#if wingetOperation}
            <p>
              Operation {wingetOperation.id}: {wingetOperation.step} ({wingetOperation.state}){wingetOperation.errorCode
                ? ` — ${wingetOperation.errorCode}`
                : ""}
            </p>
            {#if wingetOperation.state === "RUNNING"}<button
                type="button"
                disabled={wingetOperation.cancellationRequested}
                onclick={() => void cancelWinget()}
                >Cancel WinGet operation</button
              >{/if}
            {#if wingetOperation.state === "RECOVERY_REQUIRED"}<button
                type="button"
                onclick={() => void closeWinget()}
                >Keep outcome uncertain and close</button
              >{/if}
          {/if}
          {#if wingetItems.length}<h4>WinGet installations for this Cask</h4>
            <ul>
              {#each wingetItems as item (item.id)}
                <li>
                  {item.packageId}
                  {item.packageVersion} ({item.state}) — {item.registryInstallationId}
                  <p>{item.warnings.join(" ")}</p>
                  <button
                    type="button"
                    disabled={wingetBusy || !item.canPlanUninstall}
                    onclick={() => void planWingetUninstall(item)}
                    >Preview uninstall</button
                  >
                </li>
              {/each}
            </ul>{/if}
          {#if wingetPlan}
            <div class="review">
              <p>
                Uninstall plan {wingetPlan.id}: {wingetPlan.packageId}
                {wingetPlan.packageVersion}, {wingetPlan.state}. Expires {new Date(
                  wingetPlan.expiresAtMs,
                ).toLocaleString()}.
              </p>
              <p>{wingetPlan.warnings.join(" ")}</p>
              {#if wingetPlan.state === "DRAFT"}<button
                  type="button"
                  onclick={() => void approveWingetUninstall()}
                  >Approve exact plan</button
                >{/if}
              {#if wingetPlan.state === "APPROVED" && wingetCapabilities?.uninstall}
                <label for="winget-confirm"
                  >Enter plan ID to confirm uninstall</label
                ><input id="winget-confirm" bind:value={wingetConfirmation} />
                <button
                  type="button"
                  disabled={wingetBusy || wingetConfirmation !== wingetPlan.id}
                  onclick={() => void executeWingetUninstall()}
                  >Execute approved uninstall</button
                >
              {/if}
              {#if wingetPlan.state === "APPROVED" && !wingetCapabilities?.uninstall}<p
                >
                  Uninstall execution is pending independent removal review.
                </p>{/if}
            </div>
          {/if}
        </section>
      {/if}
      {#if applicationWorkflow === "installer"}
        <section aria-label="Installer artifacts">
          <h3>MSI and EXE installers</h3>
          <p>
            Register a reviewed copy before launching it. Installers are not
            sandboxed or certified by Casklet. They may change your user account
            or the whole machine; cleanup is not guaranteed.
          </p>
          <button
            type="button"
            disabled={installerBusy}
            onclick={() => void inspectInstaller()}>Inspect MSI or EXE</button
          >
          {#if installerPreview}<div class="review">
              <p><strong>Source:</strong> {installerPath}</p>
              <p>
                <strong>Format:</strong>
                {installerPreview.format}; <strong>SHA-256:</strong>
                {installerPreview.sha256}
              </p>
              <p>
                Signature: {installerPreview.signatureStatus}{installerPreview.signatureDetail
                  ? ` — ${installerPreview.signatureDetail}`
                  : ""}; signer: {installerPreview.signer ?? "Unavailable"}.
                Signature evidence does not establish safety.
              </p>
              <p>
                Product: {installerPreview.productName ?? "Unknown"}; publisher: {installerPreview.publisher ??
                  "Unknown"}; version: {installerPreview.productVersion ??
                  "Unknown"}; MSI ProductCode: {installerPreview.productCode ??
                  "Unavailable"}.
              </p>
              <p>{installerPreview.warnings.join(" ")}</p>
              <button
                type="button"
                disabled={installerBusy}
                onclick={() => void registerInstaller()}
                >Copy and register for this Cask</button
              >
            </div>{/if}
          {#if installerItems.length}<h4>Registered artifacts</h4>
            <ul>
              {#each installerItems as item (item.id)}<li>
                  {item.inspection.productName ?? item.inspection.path} ({item
                    .inspection.format}) — {item.inspection.signatureStatus}
                  <button
                    type="button"
                    disabled={installerBusy}
                    onclick={() => {
                      installerSelected = item;
                      installerAccept = false;
                      installerElevate = false;
                    }}>Review launch</button
                  >
                </li>{/each}
            </ul>{/if}
          {#if installerSelected}<div class="review">
              <h4>Launch {installerSelected.inspection.format}</h4>
              <p>Managed copy: {installerSelected.inspection.path}</p>
              <p>Original source: {installerSelected.sourcePath}</p>
              <p>SHA-256: {installerSelected.inspection.sha256}</p>
              <p>
                Signer: {installerSelected.inspection.signer ?? "Unavailable"}
                ({installerSelected.inspection
                  .signatureStatus}{installerSelected.inspection.signatureDetail
                  ? ` — ${installerSelected.inspection.signatureDetail}`
                  : ""}). Requested scope may be user or system; the installer
                and Windows decide.
              </p>
              <p>{installerSelected.inspection.warnings.join(" ")}</p>
              {#if installerSelected.inspection.format === "EXE"}<label
                  ><input type="checkbox" bind:checked={installerElevate} /> Request
                  Windows UAC elevation for this installer process</label
                >{/if}
              <label
                ><input type="checkbox" bind:checked={installerAccept} /> I reviewed
                this exact installer and approve launching it</label
              >
              <button
                type="button"
                disabled={installerBusy || !installerAccept}
                onclick={() => void launchInstaller()}
                >Launch selected installer</button
              >
            </div>{/if}
          {#if installerOperation}<div class="review">
              <p>
                Operation {installerOperation.id}: {installerOperation.state}{installerOperation.errorCode
                  ? ` — ${installerOperation.errorCode}`
                  : ""}{installerOperation.exitCode !== null
                  ? ` (exit ${installerOperation.exitCode})`
                  : ""}
              </p>
              <p>{installerOperation.warnings.join(" ")}</p>
              {#if installerOperation.state === "RUNNING"}<button
                  type="button"
                  onclick={() => void cancelInstaller()}
                  >Request cancellation</button
                >{/if}
              {#if installerOperation.candidates.length}<h4>
                  New registrations to review
                </h4>
                <ul>
                  {#each installerOperation.candidates as candidate (candidate.installationId)}<li
                    >
                      {candidate.name} — {candidate.publisher ??
                        "Unknown publisher"} ({candidate.scope}; {candidate.confidence}
                      evidence)
                      <p>{candidate.evidence.join("; ")}</p>
                      {#if installerOperation.state === "REVIEW_REQUIRED" || installerOperation.state === "RECOVERY_REQUIRED"}<button
                          type="button"
                          onclick={() =>
                            void confirmInstallerCandidate(
                              candidate.installationId,
                            )}>Confirm reference association</button
                        >{/if}
                    </li>{/each}
                </ul>{:else}<p>
                  No new Windows registration was observed. No association or
                  ownership was created.
                </p>{/if}
              {#if installerOperation.state === "REVIEW_REQUIRED" || installerOperation.state === "RECOVERY_REQUIRED"}<button
                  type="button"
                  onclick={() => void closeInstaller()}
                  >Preserve outcome and close review</button
                >{/if}
            </div>{/if}
          {#if installerOperations.length}<h4>Installer operation history</h4>
            <ul>
              {#each installerOperations as operation (operation.id)}<li>
                  {operation.id}: {operation.state}<button
                    type="button"
                    onclick={() => {
                      installerOperationId = operation.id;
                      void refreshInstallerOperation();
                    }}>Inspect</button
                  >
                </li>{/each}
            </ul>{/if}
        </section>
      {/if}
      {#if applicationWorkflow === "existing"}
        <section
          class="application-picker"
          aria-label="Existing Windows applications"
        >
          <div class="section-heading">
            <h3>Associate existing application</h3>
          </div>
          <p>
            Link an installation to this activity. Association does not grant
            ownership or uninstall permission.
          </p>
          <form
            class="picker-search"
            onsubmit={(event) => {
              event.preventDefault();
              void searchInstalled();
            }}
          >
            <label for="installed-search">Search installed applications</label>
            <div class="picker-search-row">
              <label class="search-field"
                ><Icon name="search" size={16} /><input
                  id="installed-search"
                  bind:value={installedQuery}
                  maxlength="120"
                  placeholder="Application name or publisher…"
                /></label
              ><button class="primary-button" disabled={installedBusy}
                >{installedBusy ? "Searching…" : "Search"}</button
              >
            </div>
          </form>
          <div class="picker-results-heading">
            <span
              >{installedSearched
                ? `${detectedApps.length} ${detectedApps.length === 1 ? "installation" : "installations"} found`
                : "Windows installation inventory"}</span
            ><button
              class="text-button"
              type="button"
              disabled={installedBusy}
              onclick={() => void refreshInstalled()}
              ><Icon name="refresh" size={14} />Refresh inventory</button
            >
          </div>
          {#if installedBusy}<p class="loading" role="status">
              Reading installed applications…
            </p>{:else if detectedApps.length}
            <ul class="application-list picker-results">
              {#each detectedApps as item (item.installationId)}
                <li>
                  <Icon name="package" size={18} />
                  <div class="item-text">
                    <strong title={item.name}>{item.name}</strong><span
                      >{item.publisher ?? "Unknown publisher"} · {item.displayVersion ??
                        "Version not recorded"} · {label(item.scope)}</span
                    >
                    <details class="metadata-disclosure">
                      <summary>Installation details</summary>
                      <p class="mono">{item.sourceKey}</p>
                      {#if item.installLocation}<p>
                          {item.installLocation}
                        </p>{/if}{#if item.ambiguousName}<p class="warning">
                          Other registrations use the same name. Check the
                          source before associating.
                        </p>{/if}
                    </details>
                  </div>
                  <button
                    type="button"
                    disabled={installedBusy ||
                      associatedApps.some(
                        (a) => a.installationId === item.installationId,
                      )}
                    onclick={() => void associateDetected(item)}
                    >{associatedApps.some(
                      (a) => a.installationId === item.installationId,
                    )
                      ? "Associated"
                      : "Associate"}</button
                  >
                </li>
              {/each}
            </ul>
          {:else}<div class="empty-state compact">
              <Icon name="search" size={24} />
              <h3>
                {installedSearched
                  ? "No matching applications"
                  : "Find an application"}
              </h3>
              <p>
                {installedSearched
                  ? "Try a different name, or refresh the inventory."
                  : "Search by name or publisher. Refresh to discover recent installations."}
              </p>
            </div>{/if}
        </section>
      {/if}
    {/if}
    {#if selected && detailTab === "files"}
      {#if adding || pendingPath}
        <section class="resource-intake" aria-label="Resource intake">
          <div class="section-heading">
            <h3>Add a resource</h3>
            <button
              class="icon-button"
              type="button"
              aria-label="Close resource intake"
              disabled={busy}
              onclick={() => {
                adding = false;
                pendingPath = "";
                pendingMode = "";
                referencePicker = false;
              }}><Icon name="close" size={16} /></button
            >
          </div>
          {#if !pendingPath}<p>
              Choose a file or folder, or drop one into this window.
            </p>
            <div class="resource-source-actions">
              <button type="button" onclick={() => void pick(false)}
                ><Icon name="file" size={16} />Add File</button
              ><button type="button" onclick={() => void pick(true)}
                ><Icon name="folder" size={16} />Add Folder</button
              ><button
                class="text-button"
                type="button"
                aria-expanded={referencePicker}
                onclick={() => {
                  referencePicker = !referencePicker;
                }}><Icon name="link" size={16} />Add Reference</button
              >
            </div>
            {#if referencePicker}<div class="reference-picker">
                <p>
                  Track an existing item where it is. Its source remains
                  unchanged.
                </p>
                <div class="form-row">
                  <button type="button" onclick={() => void pick(false, true)}
                    >Reference file</button
                  ><button type="button" onclick={() => void pick(true, true)}
                    >Reference folder</button
                  >
                </div>
              </div>{/if}
          {:else}
            <div class="intake-source">
              <Icon name="file" size={18} />
              <div>
                <strong title={pendingPath}
                  >{pendingPath.split(/[\\/]/).pop()}</strong
                ><span class="truncate path" title={pendingPath}
                  >{pendingPath}</span
                >
              </div>
              <button
                class="text-button"
                type="button"
                disabled={busy}
                onclick={() => {
                  pendingPath = "";
                  pendingMode = "";
                }}>Change</button
              >
            </div>
            <fieldset class="intake-modes" disabled={busy}>
              <legend>How should Casklet add it?</legend>
              <label class:chosen={pendingMode === "copy"}
                ><input
                  type="radio"
                  name="intake-mode"
                  value="copy"
                  aria-label="Copy into managed storage"
                  bind:group={pendingMode}
                /><span
                  ><strong>Copy into managed storage</strong><small
                    >Keep the original and create a managed copy.</small
                  ></span
                ></label
              >
              <label class:chosen={pendingMode === "move"}
                ><input
                  type="radio"
                  name="intake-mode"
                  value="move"
                  aria-label="Move into managed storage"
                  bind:group={pendingMode}
                /><span
                  ><strong>Move into managed storage</strong><small
                    >Verify the destination, then remove the source.</small
                  ></span
                ></label
              >
              <label class:chosen={pendingMode === "reference"}
                ><input
                  type="radio"
                  name="intake-mode"
                  value="reference"
                  aria-label="Reference at current location"
                  bind:group={pendingMode}
                /><span
                  ><strong>Reference at current location</strong><small
                    >Track the item without changing its location.</small
                  ></span
                ></label
              >
            </fieldset>
            <div class="intake-category">
              <label for="intake-category">Category</label><select
                id="intake-category"
                bind:value={pendingCategory}
                disabled={busy}
                >{#each categories as category (category.key)}<option
                    value={category.key}>{category.name}</option
                  >{/each}</select
              >
            </div>
            {#if pendingMode}<p
                class="intake-explanation"
                class:warning={pendingMode === "move"}
              >
                {pendingMode === "copy"
                  ? "A verified copy will be stored in this Cask. The original stays where it is."
                  : pendingMode === "move"
                    ? "Move changes the source location. The original is removed only after the managed destination verifies."
                    : "The item stays at its current location. Reference does not give Casklet deletion ownership."}
              </p>{:else}<p class="intake-explanation">
                Choose Copy, Move or Reference to continue.
              </p>{/if}
            <div class="intake-actions">
              <button
                class="primary-button"
                type="button"
                disabled={busy || !pendingMode}
                onclick={() => void addResource()}
                >{busy
                  ? "Adding…"
                  : pendingMode === "reference"
                    ? "Add reference"
                    : "Add resource"}</button
              >
            </div>
          {/if}
          {#if activeOperation && busy}<button
              class="text-button"
              type="button"
              onclick={() => void cancelOperation(activeOperation)}
              >Cancel operation</button
            >{/if}
        </section>
      {/if}
      <section aria-label="Resource inventory">
        <div class="section-heading">
          <h2>Files & folders</h2>
          <span class="count">{fileResources.length}</span>
        </div>
        <p class="intro">
          Managed items are stored by Casklet. References stay at their original
          location.
        </p>
        {#if fileResources.length === 0}<div class="empty-state compact">
            <Icon name="folder" size={28} />
            <h3>No files or folders yet</h3>
            <p>Use Add Items below, or drop an item into this window.</p>
          </div>{/if}
        <ul class="resource-list">
          {#each fileResources as resource (resource.id)}<li>
              <Icon
                name={resource.management === "REFERENCED"
                  ? "link"
                  : resource.kind === "DIRECTORY"
                    ? "folder"
                    : "file"}
              /><button
                class="resource-open"
                type="button"
                onclick={() => {
                  selectedResource = resource;
                }}
                ><strong title={resource.name}>{resource.name}</strong><span
                  class="truncate path"
                  title={resource.path}>{resource.path}</span
                ><span class="resource-meta"
                  >{resource.kind === "DIRECTORY" ? "Folder" : "File"} · {label(
                    resource.management,
                  )}{resource.shared ? " · Shared" : ""} · {bytes(
                    resource.sizeBytes,
                  )}</span
                ></button
              ><span
                class:warning={resource.availability !== "PRESENT"}
                class="resource-availability"
                >{label(resource.availability)}</span
              >
            </li>{/each}
        </ul>
        {#if selectedResource}<div class="review resource-inspector">
            <div class="section-heading">
              <h3 title={selectedResource.name}>{selectedResource.name}</h3>
              <button
                class="icon-button"
                type="button"
                aria-label="Close resource details"
                onclick={() => {
                  selectedResource = null;
                }}><Icon name="close" /></button
              >
            </div>
            <dl>
              <dt>Location</dt>
              <dd>{selectedResource.path}</dd>
              <dt>Management</dt>
              <dd>{label(selectedResource.management)}</dd>
              <dt>Availability</dt>
              <dd>{label(selectedResource.availability)}</dd>
              <dt>Size</dt>
              <dd>
                {bytes(selectedResource.sizeBytes)} · {label(
                  selectedResource.sizeMeasurement,
                )}
              </dd>
            </dl>
            <div class="resource-category-row">
              <label for="resource-category">Category</label><select
                id="resource-category"
                value={selectedResource.categoryKey}
                onchange={(event) => {
                  if (selectedResource)
                    void recategorize(
                      selectedResource,
                      event.currentTarget.value,
                    );
                }}
                >{#each categories as category (category.key)}<option
                    value={category.key}>{category.name}</option
                  >{/each}</select
              >
            </div>
            <div class="resource-inspector-actions">
              <button
                class="text-button"
                type="button"
                onclick={previewSelectedResource}>Preview cleanup</button
              >
            </div>
          </div>{/if}
      </section>
    {/if}
    {#if selected && detailTab === "cleanup"}
      <section aria-label="Removal preview">
        <div class="section-heading">
          <h2>Cleanup plan</h2>
          <button
            class="text-button"
            type="button"
            onclick={() => {
              detailTab = "overview";
            }}>Back to overview</button
          >
        </div>
        <p>
          Review what Casklet could detach, preserve, move to the Recycle Bin,
          or permanently delete. This preview does not change files or
          associations.
        </p>
        <label for="removal-category">Scope</label>
        <select id="removal-category" bind:value={removalCategory}>
          <option value="">Whole Cask</option>
          {#each categories as category (category.key)}<option
              value={category.key}>{category.name}</option
            >{/each}
        </select>
        <label
          ><input type="checkbox" bind:checked={permanentPlan} /> Permanently delete
          eligible managed files in this plan</label
        >
        <p>
          Permanent deletion cannot be undone. Managed directories remain
          preserved in a permanent plan; use a Recycle Bin plan for them.
        </p>
        <button type="button" onclick={() => void previewRemoval()}
          >Create cleanup preview</button
        >
        {#if selectedResource}<button
            type="button"
            onclick={previewSelectedResource}>Preview selected resource</button
          >{/if}
        {#if removalPlan}
          <p>
            Plan {removalPlan.id} is {removalPlan.state.toLowerCase()} and expires
            {new Date(removalPlan.expiresAtMs).toLocaleString()}. Disposal: {removalPlan.permanent
              ? "permanent files"
              : "Recycle Bin"}.
          </p>
          <ul>
            {#each removalPlan.items as item (item.id)}<li>
                {item.name}: {item.action.toLowerCase()} ({item.reasonCode.toLowerCase()});
                {item.state.toLowerCase()}{item.resultCode
                  ? ` (${item.resultCode.toLowerCase()})`
                  : ""}{#if item.recoveryPath}<br />Possible recovery location:
                  <code>{item.recoveryPath}</code>{/if}
              </li>{/each}
          </ul>
          {#if removalEnabled && removalPlan.state === "DRAFT"}
            <button
              type="button"
              disabled={removalBusy}
              onclick={() => void approveRemoval()}>Approve this plan</button
            >
          {/if}
          {#if removalEnabled && removalPlan.state === "APPROVED"}
            <p role="alert">
              Executing may {removalPlan.permanent
                ? "permanently delete eligible managed files"
                : "move eligible managed files and directories to the Recycle Bin"}
              and detach associations. Changed or uncertain items are preserved for
              review.
            </p>
            {#if removalPlan.permanent}<label
                ><input type="checkbox" bind:checked={permanentConfirmation} /> I
                understand that permanent deletion cannot be undone</label
              >{/if}
            <label for="removal-confirmation">Type the plan ID to execute</label
            >
            <input
              id="removal-confirmation"
              bind:value={removalConfirmation}
              autocomplete="off"
            />
            <button
              type="button"
              disabled={removalBusy ||
                removalConfirmation !== removalPlan.id ||
                (removalPlan.permanent && !permanentConfirmation)}
              onclick={() => void executeRemoval()}>Execute cleanup</button
            >
          {/if}
          {#if removalEnabled && removalPlan.state === "PARTIAL" && removalPlan.runId}
            <p role="alert">
              The cleanup outcome is uncertain. Inspect the original path,
              Recycle Bin, and any recovery location before closing. Closing
              releases the lock and keeps unresolved outcomes marked unknown.
            </p>
            <label for="removal-confirmation"
              >Type the run ID to close with preservation</label
            >
            <input
              id="removal-confirmation"
              bind:value={removalConfirmation}
              autocomplete="off"
            />
            <button
              type="button"
              disabled={removalBusy ||
                removalConfirmation !== removalPlan.runId}
              onclick={() => void closeUncertainRemoval()}
              >Close uncertain run</button
            >
          {/if}
        {/if}
      </section>
    {/if}
    {#if selected && detailTab === "settings"}
      <section aria-label="Cask settings">
        <h2>Cask settings</h2>
        <form
          onsubmit={(event) => {
            event.preventDefault();
            void saveMetadata();
          }}
        >
          <label for="edit-name">Cask name</label><input
            id="edit-name"
            bind:value={editName}
            maxlength="120"
            required
          /><label for="edit-description">Description</label><input
            id="edit-description"
            bind:value={editDescription}
            maxlength="500"
            placeholder="What is this activity for?"
          /><label for="storage-location">Storage location</label><input
            id="storage-location"
            value={selected.managedStorageRoot}
            readonly
            title={selected.managedStorageRoot}
          />
          <p class="footnote">
            Resources may also be referenced from locations outside this folder.
          </p>
          <label class="checkbox-label"
            ><input type="checkbox" bind:checked={editArchived} /> Archive this Cask</label
          >
          <p class="footnote">
            Archiving makes the Cask read-only and preserves its resources.
            Clear this option to restore it.
          </p>
          <button
            class="primary-button"
            disabled={metadataBusy || !editName.trim()}
            >{metadataBusy ? "Saving…" : "Save changes"}</button
          >
        </form>
      </section>
      <section aria-label="Categories">
        <h3>New category</h3>
        <form
          onsubmit={(event) => {
            event.preventDefault();
            void createCategory();
          }}
        >
          <label for="category-name">Name</label>
          <div class="form-row">
            <input
              id="category-name"
              bind:value={categoryName}
              maxlength="80"
            /><button>Create category</button>
          </div>
        </form>
      </section>
      <section aria-label="Export Cask">
        <h2>Export Cask</h2>
        <p class="intro">
          Portable exports exclude machine-specific paths by default.
        </p>
        <div class="action-row">
          <button type="button" onclick={() => void exportCask("json")}
            >Portable JSON</button
          ><button type="button" onclick={() => void exportCask("markdown")}
            >Export report</button
          ><button type="button" onclick={() => void exportCask("json", true)}
            >Machine JSON</button
          >
        </div>
      </section>
    {/if}
    {#if mode === "settings"}
      <section id="settings" aria-label="Settings and privacy">
        <h2>Settings and privacy</h2>
        <p>
          Casklet stores its database locally. A database backup contains
          metadata only; managed files need a separate backup. Casklet has no
          account or default telemetry.
        </p>
        <button
          type="button"
          disabled={backupBusy}
          onclick={() => void backupDatabase()}
          >{backupBusy ? "Backing up…" : "Back up database metadata"}</button
        >
        <button type="button" onclick={() => void saveDiagnostics()}
          >Save diagnostics</button
        >
        <p>
          Review diagnostics before sharing them because paths and operation
          details may be sensitive.
        </p>
        <p>
          Destructive cleanup is {removalEnabled
            ? "available after plan review"
            : "disabled pending independent security review"}.
        </p>
      </section>
    {/if}
  </div>
  {#if mode === "detail"}<footer class="workspace-footer">
      <button
        type="button"
        disabled={!selected || busy || selected.state !== "ACTIVE"}
        onclick={() => {
          detailTab = "files";
          adding = !adding;
        }}><Icon name="plus" /> Add Items</button
      ><button
        class="danger-button"
        type="button"
        disabled={!selected || removalBusy}
        onclick={() => {
          detailTab = "cleanup";
          void previewRemoval();
        }}><Icon name="trash" size={17} /> Create Cleanup Plan</button
      >
    </footer>{/if}
</div>
