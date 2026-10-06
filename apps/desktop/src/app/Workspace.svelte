<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { onMount, tick } from "svelte";
  import type {
    CaskRemovalStatus,
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
    InstallerArtifactView,
    InstallerInspection,
    InstallerOperationView,
    DatabaseBackupView,
  } from "../contracts/generated/casklet";
  import { CONTRACT_VERSION } from "../contracts/generated/casklet";
  import MsiCleanup from "./MsiCleanup.svelte";
  import ApplicationLibrary from "./ApplicationLibrary.svelte";
  import { applicationRows, type ApplicationRow } from "./applications";
  import CleanupReview from "./CleanupReview.svelte";
  import PopupMenu from "./PopupMenu.svelte";
  import { notify } from "./notifications";
  import Icon from "./Icon.svelte";
  import {
    caskColor,
    activityDay,
    bytes,
    relativeTime,
    label,
    signatureLabel,
  } from "./format";
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
  let ownershipConfirmed = $state(false);
  let pendingMode: "reference" | "adopt" | "" = $state("");
  let pendingCategory = $state("other");
  let progress = $state("");
  let categoryName = $state("");
  let selectedResource: ResourceView | null = $state(null);
  let recovery: RecoveryStatus | null = $state(null);
  let recoveryInspection: RecoveryInspection | null = $state(null);
  let activeOperation = $state("");
  let operations: OperationView[] = $state([]);
  const recentOperations = $derived(
    [...operations]
      .filter(
        (item) =>
          !["COMPLETE", "CANCELLED", "FAILED"].includes(item.state) ||
          activityHidden[item.id] !== item.updatedAtMs,
      )
      .sort((a, b) => b.updatedAtMs - a.updatedAtMs),
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
  let caskRemovalStatus: CaskRemovalStatus | null = $state(null);
  let removeConfirmation = $state(false);
  let removeDialog = $state<HTMLDialogElement>(null!);
  let activityHidden = $state<Record<string, number>>({});
  const activityKey = () => `casklet.activity.hidden.${caskId ?? "all"}`;
  function clearActivity() {
    activityHidden = Object.fromEntries(
      operations
        .filter((item) =>
          ["COMPLETE", "CANCELLED", "FAILED"].includes(item.state),
        )
        .map((item) => [item.id, item.updatedAtMs]),
    );
    try {
      localStorage.setItem(activityKey(), JSON.stringify(activityHidden));
    } catch {
      error =
        "Activity hidden for this session; your display preference could not be saved.";
    }
  }
  async function confirmCaskRemoval() {
    removeConfirmation = true;
    await tick();
    removeDialog.showModal();
    removeDialog.querySelector<HTMLButtonElement>("button")?.focus();
  }
  let removed = $state(false);
  let reviewedPlanHash = $state("");
  let executionConfirmed = $state(false);
  let cleanupResourceIds: string[] = $state([]);
  let removalPlan: RemovalPlanView | null = $state(null);
  let removalCategory = $state("");
  let removalEnabled = $state(false);
  let preserveConfirmed = $state(false);
  let permanentPlan = $state(false);
  let permanentConfirmation = $state(false);
  let removalBusy = $state(false);
  let historyPlanId = $state("");
  let cleanupHistory: RemovalPlanView[] = $state([]);
  const cleanupPending = $derived(
    cleanupHistory.some((plan) =>
      ["PARTIAL", "EXECUTING"].includes(plan.state),
    ),
  );
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
  let installerArtifacts: InstallerArtifactView[] = $state([]);
  let installerElevation = $state(false);
  let installerPath = $state("");
  let installerPreview: InstallerInspection | null = $state(null);
  let installerOperation: InstallerOperationView | null = $state(null);
  let installerOperationId = $state("");
  let installerBusy = $state(false);
  let installerKeepConfirmed = $state(false);
  let installerRefreshRevision = 0;
  let fileResources = $derived(
    resources.filter(
      (resource) =>
        !portableItems.some((item) => item.resourceId === resource.id) &&
        !installerArtifacts.some((item) => item.resourceId === resource.id),
    ),
  );
  const applications = $derived(
    applicationRows(portableItems, associatedApps, wingetItems),
  );
  const appCount = $derived(applications.length);
  const installerNeedsAttention = $derived.by(() =>
    ["RUNNING", "REVIEW_REQUIRED", "RECOVERY_REQUIRED"].includes(
      installerOperation?.state ?? "",
    ),
  );
  let applicationRemovalId = $state("");
  async function applicationAction(row: ApplicationRow) {
    if (row.kind === "portable") {
      await previewPortableRemoval(row.item);
    } else if (row.kind === "winget") {
      applicationWorkflow = "winget";
      await planWingetUninstall(row.item);
    } else if (
      ["MANAGED_MSI", "CASKLET_OWNED"].includes(row.item.removalPolicy)
    ) {
      await navigateCleanup([], row.item.installationId);
    } else {
      await disassociateDetected(row.item);
    }
  }
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
    if (installerBusy) return;
    installerBusy = true;
    installerPreview = null;
    try {
      const path = await open({
        multiple: false,
        filters: [{ name: "Windows installer", extensions: ["msi", "exe"] }],
      });
      if (typeof path !== "string") return;
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
  async function refreshInstallers() {
    if (!selected) return;
    const caskId = selected.id;
    const artifacts = await invoke<InstallerArtifactView[]>(
      "list_installer_artifacts",
      { request: { version: CONTRACT_VERSION, caskId } },
    );
    if (selected?.id !== caskId) return;
    installerArtifacts = artifacts;
    const operations = await invoke<InstallerOperationView[]>(
      "list_installer_operations",
      {
        request: { version: CONTRACT_VERSION, caskId },
      },
    );
    if (selected?.id !== caskId) return;
    const pending = operations.find((operation) =>
      ["RUNNING", "REVIEW_REQUIRED", "RECOVERY_REQUIRED"].includes(
        operation.state,
      ),
    );
    if (pending && !installerBusy && !installerNeedsAttention) {
      installerRefreshRevision += 1;
      installerOperation = pending;
      installerOperationId = pending.id;
      installerKeepConfirmed = false;
    }
  }
  async function installApplication() {
    if (!selected || !installerPreview || installerBusy) return;
    const caskId = selected.id;
    installerBusy = true;
    installerOperation = null;
    installerOperationId = "";
    error = "";
    try {
      const result = await invoke<InstallerOperationView>(
        "install_application",
        {
          request: {
            version: CONTRACT_VERSION,
            caskId,
            path: installerPath,
            expectedSha256: installerPreview.sha256,
            acceptExecutionRisk: true,
            allowElevation: installerElevation,
          },
        },
      );
      if (selected?.id !== caskId) return;
      installerRefreshRevision += 1;
      installerOperation = result;
      installerKeepConfirmed = false;
      installerOperationId = result.id;
      installerPreview = null;
      await refreshResources();
      await refreshInstallers();
      progress = result.confirmedInstallationId
        ? "Application added to this Cask."
        : "Installer finished. Check its result below.";
    } catch (cause) {
      error = message(cause);
      if (selected?.id === caskId) {
        await refreshInstallerOperation().catch(() => {});
        await refreshResources().catch(() => {});
      }
    } finally {
      installerBusy = false;
    }
  }
  function installerStatus(operation: InstallerOperationView) {
    if (operation.state === "RUNNING")
      return "Installing… Finish the installer and close any apps it opened.";
    if (operation.confirmedInstallationId)
      return "Application added to this Cask.";
    if (operation.errorCode === "REBOOT_REQUIRED")
      return "Restart Windows to finish installation. Casklet could not yet verify the application.";
    if (operation.errorCode === "ELEVATION_DECLINED")
      return "Installation cancelled. Windows permission was declined.";
    if (operation.errorCode?.includes("CANCELLED"))
      return "Installation cancelled. Some changes may remain.";
    if (operation.state === "CLOSED_PRESERVED")
      return "Installed changes have been kept. Casklet could not identify one matching application; no application was claimed.";
    return "Installation could not be confirmed. Check the Windows installer. If it has finished, you can keep its changes below.";
  }
  async function refreshInstallerOperation() {
    if (installerOperationId) {
      const operationId = installerOperationId;
      const revision = ++installerRefreshRevision;
      const result = await invoke<InstallerOperationView>(
        "get_installer_operation",
        { request: { version: CONTRACT_VERSION, id: operationId } },
      );
      if (
        revision === installerRefreshRevision &&
        operationId === installerOperationId &&
        result.caskId === selected?.id
      )
        installerOperation = result;
      installerKeepConfirmed = false;
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
    if (!installerOperation || installerBusy || !installerKeepConfirmed) return;
    installerBusy = true;
    try {
      await invoke("close_installer_operation", {
        request: { version: CONTRACT_VERSION, id: installerOperation.id },
      });
      await refreshInstallerOperation();
      await refreshInstallers();
      installerKeepConfirmed = false;
      progress =
        "Installed software has been kept. The installer will not run again.";
    } catch (cause) {
      error = message(cause);
    } finally {
      installerBusy = false;
    }
  }
  function resetInstallerReview() {
    installerRefreshRevision += 1;
    installerPreview = null;
    installerOperation = null;
    installerOperationId = "";
    installerKeepConfirmed = false;
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
      await refreshCleanupHistory();
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
    navigateCleanup([item.resourceId]);
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
  async function previewRemoval(resourceIds: string[] = cleanupResourceIds) {
    if (
      !selected ||
      removalBusy ||
      cleanupPending ||
      ["PARTIAL", "EXECUTING"].includes(removalPlan?.state ?? "")
    )
      return;
    detailTab = "cleanup";
    const scoped = resources.filter((item) =>
      resourceIds.length
        ? resourceIds.includes(item.id)
        : !removalCategory || item.categoryKey === removalCategory,
    );
    if (!scoped.length) {
      removalPlan = null;
      return;
    }
    removalBusy = true;
    historyPlanId = "";
    const previous = removalPlan?.id;
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
      permanentConfirmation = false;
      preserveConfirmed = false;
      reviewedPlanHash = "";
      executionConfirmed = false;
      progress =
        removalPlan.id === previous
          ? "Current cleanup plan loaded."
          : "Cleanup plan ready.";
      error = "";
      await tick();
      document.getElementById("cleanup-plan-heading")?.focus();
    } catch (cause) {
      error = message(cause);
    } finally {
      removalBusy = false;
    }
  }
  async function removeCask() {
    if (!selected || !removeConfirmation || metadataBusy) return;
    metadataBusy = true;
    try {
      const storageWarning = await invoke<string | null>("remove_cask", {
        request: {
          version: CONTRACT_VERSION,
          id: selected.id,
          expectedRevision: selected.revision,
          confirm: true,
        },
      });
      removeDialog?.close();
      removeConfirmation = false;
      removed = true;
      selected = null;
      removalPlan = null;
      progress = "Cask permanently removed.";
      if (storageWarning) notify(storageWarning, "info");
    } catch (cause) {
      error = message(cause);
      await refreshResources();
    } finally {
      metadataBusy = false;
    }
  }
  async function navigateCleanup(
    resourceIds: string[] = [],
    installationId = "",
  ) {
    applicationRemovalId = installationId;
    if (cleanupResourceIds.join() !== resourceIds.join()) {
      removalPlan = null;
      reviewedPlanHash = "";
      executionConfirmed = false;
    }
    cleanupResourceIds = resourceIds;
    detailTab = "cleanup";
    await tick();
    document.getElementById("cleanup-heading")?.focus();
  }
  function invalidateCleanupReview() {
    removalPlan = null;
    reviewedPlanHash = "";
    executionConfirmed = false;
    permanentConfirmation = false;
  }
  function previewSelectedResource() {
    if (selectedResource) navigateCleanup([selectedResource.id]);
  }
  async function reconcileCleanupFailure(cause: unknown) {
    executionConfirmed = false;
    permanentConfirmation = false;
    preserveConfirmed = false;
    if (removalPlan) {
      try {
        removalPlan = await invoke<RemovalPlanView>("get_removal_plan", {
          request: { version: CONTRACT_VERSION, id: removalPlan.id },
        });
        await refreshCleanupHistory();
      } catch {
        // Retain the last inspected plan and original error if persistence is unavailable.
      }
    }
    error = message(cause);
  }
  async function approveRemoval() {
    if (
      !removalPlan ||
      removalBusy ||
      !removalEnabled ||
      reviewedPlanHash !== removalPlan.planHash
    )
      return;
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
      await reconcileCleanupFailure(cause);
    } finally {
      removalBusy = false;
    }
  }
  async function executeRemoval() {
    if (
      !removalPlan ||
      removalBusy ||
      !removalEnabled ||
      !executionConfirmed ||
      reviewedPlanHash !== removalPlan.planHash ||
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
      preserveConfirmed = false;
      executionConfirmed = false;
      await refreshResources();
      await refresh();
      if (selected)
        selected = await invoke<CaskView>("get_cask", {
          request: { version: CONTRACT_VERSION, id: selected.id },
        });
      error = "";
    } catch (cause) {
      await reconcileCleanupFailure(cause);
    } finally {
      removalBusy = false;
    }
  }
  async function closeUncertainRemoval() {
    if (!removalPlan?.runId || !preserveConfirmed || removalBusy) return;
    removalBusy = true;
    try {
      removalPlan = await invoke<RemovalPlanView>("close_uncertain_removal", {
        request: {
          version: CONTRACT_VERSION,
          runId: removalPlan.runId,
        },
      });
      preserveConfirmed = false;
      await refreshResources();
      await refresh();
      if (selected)
        selected = await invoke<CaskView>("get_cask", {
          request: { version: CONTRACT_VERSION, id: selected.id },
        });
      error = "";
    } catch (cause) {
      await reconcileCleanupFailure(cause);
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
      await refreshCleanupHistory();
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
      ownershipConfirmed = false;
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
  async function loadCleanupPlan(plan: RemovalPlanView) {
    historyPlanId = plan.id;
    await navigateCleanup(
      plan.scope === "RESOURCES"
        ? plan.items.map((item) => item.resourceId)
        : [],
    );
    removalPlan = plan;
    permanentPlan = plan.permanent;
    removalCategory = plan.categoryKey ?? "";
    reviewedPlanHash = "";
    executionConfirmed = false;
    permanentConfirmation = false;
    preserveConfirmed = false;
  }
  async function refreshCleanupHistory() {
    cleanupHistory = await invoke<RemovalPlanView[]>("list_removal_plans", {
      request: { version: CONTRACT_VERSION, caskId: selected?.id ?? null },
    });
  }
  async function refreshResources() {
    if (!selected) return;
    const caskId = selected.id;
    caskRemovalStatus = await invoke<CaskRemovalStatus>("cask_removal_status", {
      request: { version: CONTRACT_VERSION, id: caskId },
    });
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
    await refreshInstallers().catch(() => {});
    await refreshAssociated().catch(() => {
      associatedApps = [];
    });
    categories = await invoke<CategoryView[]>("list_categories", {
      request: { version: CONTRACT_VERSION, caskId },
    });
    summary = await invoke<StorageSummary>("storage_summary", {
      request: { version: CONTRACT_VERSION, caskId },
    });
    await refreshCleanupHistory();
    if (!removalPlan) {
      const recoverable = cleanupHistory.find((plan) =>
        ["PARTIAL", "EXECUTING"].includes(plan.state),
      );
      if (recoverable) {
        removalPlan = recoverable;
        permanentPlan = recoverable.permanent;
        reviewedPlanHash = "";
        executionConfirmed = false;
        permanentConfirmation = false;
        preserveConfirmed = false;
      }
    }
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
    if (
      !selected ||
      !pendingPath ||
      !pendingMode ||
      (pendingMode === "adopt" && !ownershipConfirmed)
    )
      return;
    busy = true;
    error = "";
    try {
      await invoke<ResourceView>(
        pendingMode === "adopt" ? "adopt_resource" : "add_resource",
        {
          request: {
            version: CONTRACT_VERSION,
            caskId: selected.id,
            sourcePath: pendingPath,
            ...(pendingMode === "adopt"
              ? { confirmOwnership: ownershipConfirmed }
              : { mode: pendingMode }),
            categoryKey: pendingCategory,
          },
        },
      );
      pendingPath = "";
      pendingMode = "";
      ownershipConfirmed = false;
      adding = false;
      ownershipConfirmed = false;
      selectedResource = null;
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
    try {
      const saved: unknown = JSON.parse(
        localStorage.getItem(activityKey()) ?? "{}",
      );
      activityHidden =
        saved && typeof saved === "object" && !Array.isArray(saved)
          ? Object.fromEntries(
              Object.entries(saved).filter(
                ([, value]) =>
                  typeof value === "number" &&
                  Number.isFinite(value) &&
                  value >= 0,
              ),
            )
          : {};
    } catch {
      activityHidden = {};
    }
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
          ownershipConfirmed = false;
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
    {#if removed}<p role="status">Cask removed permanently.</p>
      <button type="button" onclick={() => void getCurrentWindow().close()}
        >Return to Casks</button
      >{/if}
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
      <details class="information-panel" aria-label="Cask information">
        <summary>Details</summary>
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
      </details>
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
        <div class="section-heading">
          <h2>Activity</h2>
          <button
            class="secondary-button"
            type="button"
            onclick={clearActivity}
            disabled={!recentOperations.some((item) =>
              ["COMPLETE", "CANCELLED", "FAILED"].includes(item.state),
            )}>Clear history</button
          >
        </div>
        <p class="footnote">
          Clears completed activity from this view. Ongoing work and recovery
          records remain available.
        </p>
        {#if recentOperations.length === 0}<div class="empty-state compact">
            <Icon name="activity" size={24} />
            <h3>No recent activity</h3>
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
      {#if cleanupHistory.some( (plan) => ["PARTIAL", "EXECUTING"].includes(plan.state), )}
        <section aria-label="Cleanup recovery">
          <h2>Cleanup needs attention</h2>
          <p>Inspect interrupted cleanup before starting more work.</p>
          {#each cleanupHistory.filter( (plan) => ["PARTIAL", "EXECUTING"].includes(plan.state), ) as plan (plan.id)}
            <button
              type="button"
              onclick={() => {
                if (selected) void loadCleanupPlan(plan);
                else
                  void invoke("open_cask_window", {
                    request: { version: CONTRACT_VERSION, id: plan.caskId },
                  });
              }}>Inspect cleanup</button
            >
          {/each}
        </section>
      {/if}
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
      <ApplicationLibrary
        rows={applications}
        workflow={applicationWorkflow}
        busy={installerBusy || portableBusy || wingetBusy || installedBusy}
        needsAttention={installerNeedsAttention}
        installationRunning={installerOperation?.state === "RUNNING"}
        onWorkflow={(value) => {
          applicationWorkflow = value;
        }}
        onAction={(row) => void applicationAction(row)}
        onLaunch={(row) => {
          if (row.kind === "portable") void launchPortable(row.item);
        }}
      >
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
                  disabled={portableBusy ||
                    !portableExe ||
                    !portableName.trim()}
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
                    disabled={wingetBusy ||
                      wingetConfirmation !== wingetPlan.id}
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
          <section aria-label="Install Application" aria-busy={installerBusy}>
            <p>
              Choose an installer. Windows will guide you through installation.
            </p>
            <button
              type="button"
              disabled={installerBusy || installerNeedsAttention}
              onclick={() => void inspectInstaller()}
            >
              {installerBusy
                ? "Working…"
                : installerPreview
                  ? "Choose another installer"
                  : "Select MSI or EXE"}
            </button>
            {#if installerPreview && !installerNeedsAttention}
              <div class="review installer-selection">
                <div class="application-icon">
                  <Icon name="download" size={22} />
                </div>
                <div>
                  <h4>
                    {installerPreview.productName ??
                      installerPath.split(/[\\/]/).pop() ??
                      "Selected installer"}
                  </h4>
                  <p>
                    {signatureLabel(installerPreview.signatureStatus)} · Only install
                    software you trust.
                  </p>
                  <p>
                    New installations belong to this Cask. Cleanup uses the
                    verified Windows uninstaller; unsupported or ambiguous
                    results remain visible for review.
                  </p>
                </div>
                <button
                  type="button"
                  class="primary"
                  disabled={installerBusy}
                  onclick={() => void installApplication()}
                  >{installerBusy ? "Installing…" : "Install"}</button
                >
              </div>
              <label class="check"
                ><input
                  type="checkbox"
                  bind:checked={installerElevation}
                  disabled={installerBusy}
                /> Request administrator privileges</label
              >
            {/if}
            {#if installerOperation}
              <div class="installation-result" role="status">
                <Icon
                  name={installerOperation.confirmedInstallationId
                    ? "check"
                    : installerOperation.state === "RUNNING"
                      ? "clock"
                      : "info"}
                  size={20}
                />
                <div>
                  <p>{installerStatus(installerOperation)}</p>
                  {#if installerOperation.confirmedInstallationId}<p
                      class="muted"
                    >
                      {associatedApps.some(
                        (item) =>
                          item.installationId ===
                            installerOperation?.confirmedInstallationId &&
                          ["MANAGED_MSI", "CASKLET_OWNED"].includes(
                            item.removalPolicy,
                          ),
                      )
                        ? "Included in approved Cask cleanup."
                        : "Remove this software through Windows when you no longer need it."}
                    </p>{/if}
                </div>
              </div>
              {#if installerOperation.state === "RUNNING"}<button
                  type="button"
                  onclick={() => void cancelInstaller()}
                  >Cancel installation</button
                >
              {:else if installerNeedsAttention}
                <div class="installer-attention-actions">
                  <p>
                    Keeping changes leaves software installed and never runs the
                    installer again.
                  </p>
                  <label class="checkbox-label"
                    ><input
                      type="checkbox"
                      bind:checked={installerKeepConfirmed}
                      disabled={installerBusy}
                    />The installer and any apps it opened have finished.</label
                  >
                  <button
                    type="button"
                    disabled={installerBusy || !installerKeepConfirmed}
                    onclick={() => void closeInstaller()}
                    >Keep installed changes</button
                  >
                </div>
              {/if}
            {/if}
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
              <label for="installed-search">Search installed applications</label
              >
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
      </ApplicationLibrary>
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
                ownershipConfirmed = false;
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
                  ownershipConfirmed = false;
                }}>Change</button
              >
            </div>
            <fieldset class="intake-modes" disabled={busy}>
              <legend>How should Casklet add it?</legend>
              <!-- ADR 0020: Copy/Move are temporarily hidden; shared user intake rejects them. Legacy/provider intake remains intact. -->
              <label class:chosen={pendingMode === "adopt"}
                ><input
                  type="radio"
                  name="intake-mode"
                  value="adopt"
                  aria-label="Manage at current location"
                  bind:group={pendingMode}
                /><span
                  ><strong>Manage at current location</strong><small
                    >Leave data here and explicitly grant cleanup authority.</small
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
            {#if pendingMode}<p class="intake-explanation">
                {pendingMode === "adopt"
                  ? "The item stays here. Cleanup requires your ownership confirmation and an approved plan."
                  : "The item stays here. Referencing never grants permission to delete it."}
              </p>{/if}
            {#if pendingMode === "adopt"}<label class="checkbox-label"
                ><input type="checkbox" bind:checked={ownershipConfirmed} /> This
                data is dedicated to this Cask, is not needed elsewhere, and may
                be moved to the Recycle Bin after I approve a cleanup plan.</label
              >{/if}
            <div class="intake-actions">
              <button
                class="primary-button"
                type="button"
                disabled={busy ||
                  !pendingMode ||
                  (pendingMode === "adopt" && !ownershipConfirmed)}
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
          Expand an item to see its location and management details.
        </p>
        {#if fileResources.length === 0}<div class="empty-state compact">
            <Icon name="folder" size={28} />
            <h3>No files or folders yet</h3>
            <p>Use Add Items below, or drop an item into this window.</p>
          </div>{/if}
        <ul class="resource-list">
          {#each fileResources as resource (resource.id)}<li
              class="resource-entry"
            >
              <button
                class="resource-toggle"
                type="button"
                aria-label={`Details for ${resource.name}`}
                aria-expanded={selectedResource?.id === resource.id}
                aria-controls={`resource-details-${resource.id}`}
                onclick={() => {
                  selectedResource =
                    selectedResource?.id === resource.id ? null : resource;
                }}
              >
                <Icon
                  name={resource.kind === "DIRECTORY" ? "folder" : "file"}
                />
                <strong title={resource.name}>{resource.name}</strong>
                {#if resource.availability !== "PRESENT"}<span
                    class="resource-state warning"
                    >{label(resource.availability)}</span
                  >{/if}
                <span class="resource-size">{bytes(resource.sizeBytes)}</span>
                <span
                  class:expanded={selectedResource?.id === resource.id}
                  class="resource-chevron"><Icon name="down" size={16} /></span
                >
              </button>
              {#if selectedResource?.id === resource.id}<div
                  class="resource-inspector resource-details"
                  id={`resource-details-${resource.id}`}
                  role="region"
                  aria-label={`Details for ${resource.name}`}
                >
                  <div class="section-heading">
                    <h3 title={selectedResource.name}>
                      {selectedResource.name}
                    </h3>
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
                    <dd>
                      {selectedResource.management === "MANAGED" &&
                      selectedResource.containment === "EXTERNAL"
                        ? "Managed in place"
                        : label(selectedResource.management)}
                    </dd>
                    <dt>Cleanup authority</dt>
                    <dd>
                      {selectedResource.ownership === "OWNED"
                        ? "Owned; requires a current reviewed plan"
                        : "Reference only; underlying data is preserved"}
                    </dd>
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
                      class="secondary-button"
                      type="button"
                      onclick={previewSelectedResource}
                      >Cleanup this item</button
                    >
                  </div>
                </div>{/if}
            </li>{/each}
        </ul>
      </section>
    {/if}
    {#if selected && detailTab === "cleanup"}
      <section aria-label="Cleanup Cask">
        <div class="section-heading">
          <h2 id="cleanup-heading" tabindex="-1">Cleanup Cask</h2>
          <button
            class="secondary-button cleanup-back"
            type="button"
            onclick={() => {
              detailTab = "overview";
            }}><Icon name="back" size={16} /> Back to Overview</button
          >
        </div>
        <MsiCleanup
          caskId={selected.id}
          enabled={removalEnabled}
          installationId={applicationRemovalId}
          hasManaged={associatedApps.length > 0}
          onChanged={refreshResources}
        />
        {#if caskRemovalStatus?.canRemove}
          <p>
            This Cask has no remaining items or unfinished work. You can remove
            it permanently.
          </p>
          <button type="button" onclick={() => void confirmCaskRemoval()}
            >Remove Cask</button
          >
        {:else}
          {#if !removalPlan}<p class="cleanup-intro">
              Clean up items, then remove the empty Cask.
            </p>{/if}
          {#if !removalEnabled && resources.length > 0}<p
              class="cleanup-notice"
              id="cleanup-limit"
            >
              <Icon name="info" size={16} /><span
                ><strong>Review only</strong> · Execution is unavailable in this
                build.</span
              >
            </p>{/if}
          {#if (caskRemovalStatus?.applicationCount ?? 0) > associatedApps.filter( (item) => ["MANAGED_MSI", "CASKLET_OWNED"].includes(item.removalPolicy), ).length}<p
            >
              Associated or unsupported applications remain. Keep and detach
              their references, or use their provider/Windows uninstall options
              before removing the Cask.
            </p>
            <button
              type="button"
              onclick={() => {
                detailTab = "apps";
              }}>Review applications</button
            >{/if}
          {#if (caskRemovalStatus?.unfinishedOperations ?? 0) > 0}<p>
              Finish or resolve outstanding work in Activity before removing
              this Cask.
            </p>{/if}
          {#if resources.length > 0}
            {#if selected.state === "ARCHIVED"}<p>
                Restore this Cask in Settings before creating a cleanup plan.
              </p>{/if}
            <label for="removal-category">Cleanup scope</label>
            <select
              id="removal-category"
              bind:value={removalCategory}
              disabled={removalBusy || cleanupPending}
              onchange={() => {
                cleanupResourceIds = [];
                invalidateCleanupReview();
              }}
            >
              <option value="">Whole Cask</option>
              {#each categories as category (category.key)}<option
                  value={category.key}>{category.name}</option
                >{/each}
            </select>
            {#if cleanupResourceIds.length}<p>
                Considering the selected item only.
              </p>
              <button
                type="button"
                onclick={() => {
                  cleanupResourceIds = [];
                  invalidateCleanupReview();
                }}
                disabled={removalBusy}>Consider Whole Cask</button
              >{/if}
            <fieldset class="cleanup-method">
              <legend>Deletion method</legend>
              <label
                ><input
                  type="radio"
                  name="deletion-method"
                  disabled={removalBusy || cleanupPending}
                  value={false}
                  bind:group={permanentPlan}
                  onchange={invalidateCleanupReview}
                /> Move to Recycle Bin</label
              >
              <label
                ><input
                  type="radio"
                  name="deletion-method"
                  disabled={removalBusy || cleanupPending}
                  value={true}
                  bind:group={permanentPlan}
                  onchange={invalidateCleanupReview}
                /> Permanently delete eligible files</label
              >
              {#if permanentPlan}<p class="method-warning">
                  Cannot be undone. Folders and files managed at their current
                  location will be kept. Execution requires an extra
                  confirmation.
                </p>{/if}
            </fieldset>
            <button
              type="button"
              onclick={() => void previewRemoval()}
              disabled={associatedApps.some((item) =>
                ["MANAGED_MSI", "CASKLET_OWNED"].includes(item.removalPolicy),
              ) ||
                removalBusy ||
                cleanupPending ||
                selected.state !== "ACTIVE" ||
                ["PARTIAL", "EXECUTING"].includes(removalPlan?.state ?? "")}
              >Create cleanup plan</button
            >
          {:else}<p>
              No file resources to clean up. Resolve the remaining dependencies
              before removing the Cask.
            </p>{/if}
        {/if}
        {#if removalPlan && !historyPlanId}
          <CleanupReview
            plan={removalPlan}
            {removalEnabled}
            {removalBusy}
            {reviewedPlanHash}
            bind:executionConfirmed
            bind:permanentConfirmation
            bind:preserveConfirmed
            onReview={() => {
              reviewedPlanHash = removalPlan!.planHash;
              executionConfirmed = false;
              permanentConfirmation = false;
            }}
            onApprove={() => void approveRemoval()}
            onExecute={() => void executeRemoval()}
            onClose={() => void closeUncertainRemoval()}
          />
        {/if}
        {#if cleanupHistory.length}
          <details class="cleanup-details">
            <summary>Recent cleanups</summary>
            {#each cleanupHistory as plan (plan.id)}
              <button
                type="button"
                disabled={removalBusy}
                onclick={() => void loadCleanupPlan(plan)}
                >{new Date(plan.createdAtMs).toLocaleDateString()} · {label(
                  plan.state,
                )} · {plan.items.length} items</button
              >
            {/each}
            {#if removalPlan && historyPlanId}
              <CleanupReview
                plan={removalPlan}
                {removalEnabled}
                {removalBusy}
                {reviewedPlanHash}
                bind:executionConfirmed
                bind:permanentConfirmation
                bind:preserveConfirmed
                onReview={() => {
                  reviewedPlanHash = removalPlan!.planHash;
                  executionConfirmed = false;
                  permanentConfirmation = false;
                }}
                onApprove={() => void approveRemoval()}
                onExecute={() => void executeRemoval()}
                onClose={() => void closeUncertainRemoval()}
              />
            {/if}
          </details>
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
          />
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
            ? "available after plan review, approval, and confirmation"
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
      >
      <button
        type="button"
        disabled={!selected || removalBusy}
        class="danger-button"
        onclick={() => navigateCleanup()}
        ><Icon name="trash" size={17} /> Cleanup</button
      >
    </footer>{/if}
</div>

{#if removeConfirmation && selected}
  <dialog
    bind:this={removeDialog}
    class="create-dialog remove-dialog"
    aria-labelledby="remove-cask-title"
    oncancel={(event) => {
      if (metadataBusy) event.preventDefault();
      else removeConfirmation = false;
    }}
  >
    <h2 id="remove-cask-title">Permanently remove Cask?</h2>
    <p>
      <strong>{selected.name}</strong> will be removed permanently. Its empty storage
      folder will also be removed. Resource files and installed applications are
      not deleted by this action. Remaining dependencies or unfinished work block
      removal.
    </p>
    <div class="dialog-actions">
      <button
        class="secondary-button"
        type="button"
        disabled={metadataBusy}
        onclick={() => {
          removeDialog.close();
          removeConfirmation = false;
        }}>Cancel</button
      ><button
        class="danger-button"
        type="button"
        disabled={metadataBusy}
        onclick={() => void removeCask()}>Remove Cask</button
      >
    </div>
  </dialog>
{/if}
