<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import ThemeSettings from "./ThemeSettings.svelte";
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
    ResourceContents,
    StorageSummary,
    RecoveryStatus,
    RecoveryInspection,
    OperationView,
    RemovalPlanView,
    CategoryCleanupPlan,
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

  import { resourceGroups, groupSize, categoryIcon } from "./resourceGroups";
  import { addResourceBatch } from "./resourceIntake";
  import ApplicationLibrary from "./ApplicationLibrary.svelte";
  import ApplicationIcon from "./ApplicationIcon.svelte";
  import { applicationRows, type ApplicationRow } from "./applications";
  import CleanupReview from "./CleanupReview.svelte";
  import PopupMenu from "./PopupMenu.svelte";
  import { notify } from "./notifications";
  import Icon from "./Icon.svelte";
  import {
    caskColor,
    activityDay,
    bytes,
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
  let dragActive = $state(false);
  function clearDragFeedback() {
    dragActive = false;
  }
  function cancelDragFeedback(event: KeyboardEvent) {
    if (event.key === "Escape") clearDragFeedback();
  }
  let adding = $state(false);
  let explorerInstaller = $state("");
  let waitingExplorer: {
    id: number;
    path: string;
    paths?: string[];
    isInstaller: boolean;
    installDirect?: boolean;
  } | null = $state(null);
  let checkingExplorer = false;
  async function receiveExplorer() {
    if (checkingExplorer || loading || !selected || removed) return;
    checkingExplorer = true;
    try {
      const request = await invoke<{
        id: number;
        path: string;
        paths?: string[];
        isInstaller: boolean;
        installDirect?: boolean;
      } | null>("explorer_pending");
      if (
        !request ||
        typeof request.id !== "number" ||
        typeof request.path !== "string"
      )
        return;
      waitingExplorer = request;
      if (
        busy ||
        removalBusy ||
        installerBusy ||
        installerBlocksMutation ||
        pendingPath ||
        adding ||
        applicationWorkflow ||
        explorerInstaller
      )
        return;
      await invoke("explorer_ack", { id: request.id });
      waitingExplorer = null;
      if (request.installDirect && request.isInstaller) {
        explorerInstaller = request.path;
        await installExplorerItem();
      } else if (request.isInstaller) explorerInstaller = request.path;
      else reviewDroppedPaths(request.paths ?? [request.path]);
    } catch (cause) {
      error = message(cause);
    } finally {
      checkingExplorer = false;
    }
  }
  async function installExplorerItem() {
    const path = explorerInstaller;
    explorerInstaller = "";
    detailTab = "apps";
    applicationWorkflow = "installer";
    installerElevation = false;
    await inspectInstaller(path);
  }
  let applicationWorkflow:
    | "portable"
    | "winget"
    | "installer"
    | "existing"
    | null = $state(null);
  function reviewDroppedPaths(paths: string[]) {
    clearDragFeedback();
    if (!paths.length) return;
    if (busy || removalBusy || installerBlocksMutation || pendingPath) {
      error =
        "Finish or close the current review before adding another selection.";
      return;
    }
    if (!selected || removed) return;
    detailTab = "files";
    adding = true;
    pendingPaths = paths;
    pendingCategory = "auto";
    intakeFailures = [];
    pendingMode = "";
    ownershipConfirmed = false;
  }
  let editName = $state("");
  let editDescription = $state("");
  let editArchived = $state(false);
  let metadataBusy = $state(false);
  let selected: CaskView | null = $state(null);
  const settingsChanged = $derived.by(
    () =>
      !!selected &&
      (editName !== selected.name ||
        editDescription !== (selected.description ?? "") ||
        editArchived !== (selected.state === "ARCHIVED")),
  );
  let contentElement: HTMLDivElement;
  $effect(() => {
    if (detailTab)
      void tick().then(() => contentElement?.scrollTo?.({ top: 0 }));
  });
  let detailTab: "overview" | "files" | "apps" | "settings" | "cleanup" =
    $state("overview");
  let error = $state("");
  let busy = $state(false);
  let backupBusy = $state(false);
  let resources: ResourceView[] = $state([]);
  let categories: CategoryView[] = $state([]);
  let summary: StorageSummary | null = $state(null);
  let pendingPaths: string[] = $state([]);
  const pendingPath = $derived(pendingPaths[0] ?? "");
  let intakeResult = $state("");
  let batchProgress = $state("");
  let intakeFailures: { path: string; reason: string }[] = $state([]);
  let ownershipConfirmed = $state(false);
  let pendingMode: "reference" | "adopt" | "" = $state("");
  let categoryEditor = $state(false);
  let categoryChoice = $state("");
  let categoryBusy = $state(false);
  let ownershipEditor = $state(false);
  let ownershipTarget = $state(false);
  let policyConfirmed = $state(false);
  let pendingCategory = $state("auto");
  let progress = $state("");
  let categoryName = $state("");
  let categoryCreating = $state(false);
  let categoryNotice = $state("");
  let categoryError = $state("");
  async function focusControl(id: string) {
    await tick();
    const element = document.getElementById(id);
    const group = element?.closest<HTMLDetailsElement>(".resource-category");
    if (group) group.open = true;
    element?.focus();
  }
  function cancelManagementEditor(event: KeyboardEvent) {
    if (event.key !== "Escape" || !(event.target instanceof HTMLElement))
      return;
    if (
      categoryEditor &&
      !categoryBusy &&
      event.target.closest("#category-editor")
    ) {
      event.preventDefault();
      event.stopPropagation();
      categoryEditor = false;
      void focusControl("change-category");
    } else if (
      ownershipEditor &&
      !busy &&
      event.target.closest("#ownership-editor")
    ) {
      event.preventDefault();
      event.stopPropagation();
      ownershipEditor = false;
      policyConfirmed = false;
      void focusControl("change-ownership");
    }
  }
  let selectedResource: ResourceView | null = $state(null);
  let overlayError = $state("");
  let overlayStatus = $state("");
  function closeResourceOverlay() {
    if (categoryBusy || (ownershipEditor && busy)) return;
    const id = selectedResource?.id;
    selectedResource = null;
    categoryEditor = false;
    ownershipEditor = false;
    policyConfirmed = false;
    folderContents = null;
    if (id) void focusControl(`resource-toggle-${id}`);
  }
  function mountResourceOverlay(node: HTMLDialogElement) {
    const wrapFocus = (event: KeyboardEvent) => {
      if (event.key !== "Tab") return;
      const controls = Array.from(
        node.querySelectorAll<HTMLElement>(
          'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], summary, [tabindex="0"]',
        ),
      ).filter(
        (control) =>
          control.getClientRects().length &&
          getComputedStyle(control).visibility !== "hidden",
      );
      const first = controls[0];
      const last = controls.at(-1);
      if (
        first &&
        last &&
        ((event.shiftKey && document.activeElement === first) ||
          (!event.shiftKey && document.activeElement === last))
      ) {
        event.preventDefault();
        (event.shiftKey ? last : first).focus();
      }
    };
    node.addEventListener("keydown", wrapFocus);
    node.showModal();
    return {
      destroy() {
        node.removeEventListener("keydown", wrapFocus);
        if (node.open) node.close();
      },
    };
  }
  function cancelResourceOverlay(event: Event) {
    event.preventDefault();
    if (categoryBusy || (ownershipEditor && busy)) return;
    if (categoryEditor) {
      categoryEditor = false;
      void focusControl("change-category");
    } else if (ownershipEditor) {
      ownershipEditor = false;
      policyConfirmed = false;
      void focusControl("change-ownership");
    } else closeResourceOverlay();
  }

  let recovery: RecoveryStatus | null = $state(null);
  let recoveryInspection: RecoveryInspection | null = $state(null);
  let activeOperation = $state("");
  let operations: OperationView[] = $state([]);
  let activityCasks: CaskView[] = $state([]);
  function activityDescription(operation: OperationView) {
    const action =
      operation.step === "INSTALL_APPLICATION"
        ? "Application installation"
        : label(operation.step);
    if (operation.state === "CANCELLED")
      return `${action} was cancelled by the user.`;
    if (
      ["COMPLETE", "SUCCEEDED", "CONFIRMED_ASSOCIATION"].includes(
        operation.state,
      )
    )
      return `${action} completed successfully.`;
    if (operation.state === "CLOSED_PRESERVED")
      return `${action} was closed with changes preserved.`;
    if (operation.state === "RUNNING") return `${action} is in progress.`;
    return `${action} needs review${operation.errorCode ? `: ${label(operation.errorCode)}` : ""}.`;
  }
  const recentOperations = $derived(
    [...operations]
      .filter(
        (item) =>
          ![
            "COMPLETE",
            "SUCCEEDED",
            "CANCELLED",
            "FAILED",
            "CLOSED_PRESERVED",
            "CONFIRMED_ASSOCIATION",
          ].includes(item.state) ||
          activityHidden[item.id] !== item.updatedAtMs,
      )
      .sort((a, b) => b.updatedAtMs - a.updatedAtMs),
  );
  $effect(() => {
    if (error) {
      if (selectedResource && detailTab === "files") overlayError = error;
      else notify(error, "error", "workspace-error");
      error = "";
    }
  });
  $effect(() => {
    if (progress) {
      if (selectedResource && detailTab === "files") {
        overlayStatus = progress;
        overlayError = "";
      } else
        notify(
          progress,
          /entries inspected|bytes processed/.test(progress)
            ? "info"
            : "success",
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
          [
            "COMPLETE",
            "SUCCEEDED",
            "CANCELLED",
            "FAILED",
            "CLOSED_PRESERVED",
            "CONFIRMED_ASSOCIATION",
          ].includes(item.state),
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
  const cleanupPending = $derived.by(
    () =>
      cleanupHistory.some((plan) =>
        ["PARTIAL", "EXECUTING"].includes(plan.state),
      ) ||
      ["PARTIAL", "EXECUTING"].includes(
        categoryCleanup?.applications?.state ?? "",
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
  let installedLinking = $state(false);
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
  let installerRefreshRevision = 0;
  let fileResources = $derived(
    resources.filter(
      (resource) =>
        !portableItems.some((item) => item.resourceId === resource.id) &&
        !installerArtifacts.some((item) => item.resourceId === resource.id),
    ),
  );
  const categoryGroups = $derived(resourceGroups(fileResources, categories));
  const applications = $derived(
    applicationRows(portableItems, associatedApps, wingetItems).map((row) => {
      const sourceKey =
        row.iconSource?.kind === "registry" ? row.iconSource.key : undefined;
      const snapshot = summary?.applicationSizes?.find(
        (size) => size.sourceKey === sourceKey,
      );
      return {
        ...row,
        sizeBytes:
          row.kind === "portable"
            ? (resources.find((resource) => resource.id === row.item.resourceId)
                ?.sizeBytes ?? null)
            : (snapshot?.sizeBytes ?? null),
        sizeEstimated: snapshot?.estimated ?? false,
      };
    }),
  );
  const appCount = $derived(applications.length);
  const installerNeedsAttention = $derived.by(() =>
    ["RUNNING", "REVIEW_REQUIRED", "RECOVERY_REQUIRED"].includes(
      installerOperation?.state ?? "",
    ),
  );
  const installerBlocksMutation = $derived.by(() =>
    ["RUNNING", "RECOVERY_REQUIRED"].includes(installerOperation?.state ?? ""),
  );
  const installerConfirmationAttempts = new SvelteSet<string>();
  let categoryCleanup: CategoryCleanupPlan | null = $state(null);
  const categoryReview = $derived.by((): RemovalPlanView | null => {
    if (!categoryCleanup) return null;
    const file = categoryCleanup.resources;
    const app = categoryCleanup.applications;
    const states = [file?.state, app?.state].filter(Boolean);
    const state = states.includes("PARTIAL")
      ? "PARTIAL"
      : states.includes("EXECUTING")
        ? "EXECUTING"
        : states.includes("DRAFT")
          ? "DRAFT"
          : states.includes("APPROVED")
            ? "APPROVED"
            : "COMPLETE";
    return {
      version: CONTRACT_VERSION,
      id: `category:${file?.id ?? ""}:${app?.id ?? ""}`,
      caskId: categoryCleanup.caskId,
      scope: applicationRemovalId
        ? "APPLICATION"
        : removalCategory
          ? "CATEGORY"
          : "CASK",
      categoryKey: removalCategory || null,
      permanent: file?.permanent ?? false,
      cleanupOnly: true,
      state:
        state === "COMPLETE" &&
        categoryCleanup.caskRemovalId &&
        !categoryCleanup.caskRemoved
          ? "APPROVED"
          : state,
      planHash: `${file?.planHash ?? ""}|${app?.planHash ?? ""}${categoryCleanup.caskRemovalId ? `|remove:${categoryCleanup.caskRemovalId}` : ""}`,
      runId: file?.runId ?? null,
      createdAtMs: file?.createdAtMs ?? Date.now(),
      expiresAtMs: Math.min(
        file?.state === "COMPLETE" ? Infinity : (file?.expiresAtMs ?? Infinity),
        app?.state === "COMPLETE" ? Infinity : (app?.expiresAtMs ?? Infinity),
      ),
      items: [
        ...(file?.items ?? []),
        ...(app?.items ?? []).map((item) => ({
          id: item.installationId,
          resourceId: item.installationId,
          name: item.name,
          path: "",
          action: item.action,
          reasonCode: "APPLICATION_OWNERSHIP",
          expectedSizeBytes: null,
          state: item.state,
          resultCode: item.result,
          recoveryPath: null,
        })),
      ],
    };
  });
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
      (summary?.referencedLogicalBytes ?? 0) +
      (summary?.ownedApplicationBytes ?? 0) +
      (summary?.referenceApplicationBytes ?? 0),
  );
  let installerSourceWarning = $state("");
  let installerRecoveryConfirmed = $state(false);
  let recoveryAttemptId = $state<string | null>(null);
  $effect(() => {
    const id = installerOperation?.id ?? null;
    if (id !== recoveryAttemptId) {
      recoveryAttemptId = id;
      installerRecoveryConfirmed = false;
    }
  });
  async function closeInstallerAttempt() {
    if (
      !installerOperation ||
      !installerRecoveryConfirmed ||
      installerBusy ||
      !["REVIEW_REQUIRED", "RECOVERY_REQUIRED"].includes(
        installerOperation.state,
      )
    )
      return;
    const id = installerOperation.id;
    installerBusy = true;
    try {
      await invoke("close_installer_operation", {
        request: { version: CONTRACT_VERSION, id },
      });
      await refreshInstallerOperation();
      await refreshResources();
      installerRecoveryConfirmed = false;
      progress =
        "Installation attempt closed. Files and applications were kept; the Cask is available again.";
    } catch (cause) {
      error = message(cause);
    } finally {
      installerBusy = false;
    }
  }
  async function inspectInstaller(source?: string) {
    if (installerBusy) return;
    installerBusy = true;
    installerPreview = null;
    installerSourceWarning = "";
    try {
      const path =
        source ??
        (await open({
          multiple: false,
          filters: [{ name: "Windows installer", extensions: ["msi", "exe"] }],
        }));
      if (typeof path !== "string") return;
      installerPreview = await invoke<InstallerInspection>(
        "inspect_installer_artifact",
        { request: { version: CONTRACT_VERSION, path } },
      );
      installerPath = path;
      error = "";
    } catch (cause) {
      if (
        (cause as Partial<ApiError>)?.code ===
        "INSTALLED_APPLICATION_EXECUTABLE"
      )
        installerSourceWarning = message(cause);
      else error = message(cause);
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
    let pending = operations.find((operation) =>
      ["RUNNING", "REVIEW_REQUIRED", "RECOVERY_REQUIRED"].includes(
        operation.state,
      ),
    );
    if (
      pending?.state === "REVIEW_REQUIRED" &&
      pending.exitCode === 0 &&
      pending.candidates.length === 1 &&
      ["HIGH", "MEDIUM"].includes(pending.candidates[0].confidence) &&
      !installerConfirmationAttempts.has(pending.id) &&
      !installerBusy
    ) {
      installerConfirmationAttempts.add(pending.id);
      try {
        await invoke("claim_managed_msi", {
          request: {
            version: CONTRACT_VERSION,
            operationId: pending.id,
            installationId: pending.candidates[0].installationId,
          },
        });
        pending = await invoke<InstallerOperationView>(
          "get_installer_operation",
          {
            request: { version: CONTRACT_VERSION, id: pending.id },
          },
        );
      } catch (cause) {
        // Preserve the evidence and surface a genuine confirmation issue.
        error = message(cause);
      }
      if (selected?.id !== caskId) return;
    }
    if (pending && !installerBusy && !installerNeedsAttention) {
      installerRefreshRevision += 1;
      installerOperation = pending;
      installerOperationId = pending.id;
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
      installerOperationId = result.id;
      installerPreview = null;
      await refreshResources();
      await refreshInstallers();
      progress = result.confirmedInstallationId
        ? "Application added to this Cask."
        : "Installer finished. Check its result below.";
    } catch (cause) {
      if (
        (cause as Partial<ApiError>)?.code ===
        "INSTALLED_APPLICATION_EXECUTABLE"
      ) {
        installerSourceWarning = message(cause);
        installerPreview = null;
      } else error = message(cause);
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
      return "Installation could not be confirmed. Check the Windows installer.";
    if (operation.state === "CLOSED_PRESERVED")
      return "Installed changes have been kept. Casklet could not identify one matching application; no application was claimed.";
    return "Installation could not be confirmed. Check the Windows installer.";
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
  function resetInstallerReview() {
    installerRefreshRevision += 1;
    installerPreview = null;
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
    if (installedBusy) return;
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
    if (installedBusy) return;
    installedBusy = true;
    try {
      const count = await invoke<number>("refresh_installed_applications", {
        request: { version: CONTRACT_VERSION },
      });
      progress = `Reconciled ${count} detected installations.`;
      detectedApps = await invoke<DetectedApplicationView[]>(
        "search_installed_applications",
        {
          request: { version: CONTRACT_VERSION, query: installedQuery },
        },
      );
      installedSearched = true;
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
  let associationSummaryRevision = 0;
  async function refreshAssociationSummary(caskId: string) {
    const revision = ++associationSummaryRevision;
    try {
      const value = await invoke<StorageSummary>("storage_summary", {
        request: { version: CONTRACT_VERSION, caskId },
      });
      if (selected?.id === caskId && revision === associationSummaryRevision)
        summary = value;
    } catch (cause) {
      if (selected?.id === caskId && revision === associationSummaryRevision)
        notify(
          `Application list updated, but storage size could not refresh: ${message(cause)}`,
          "warning",
        );
    }
  }
  async function associateDetected(item: DetectedApplicationView) {
    if (!selected || installedBusy) return;
    installedBusy = true;
    installedLinking = true;
    try {
      await invoke("associate_installed_application", {
        request: {
          version: CONTRACT_VERSION,
          caskId: selected.id,
          installationId: item.installationId,
        },
      });
      await refreshAssociated();
      void refreshAssociationSummary(selected.id);
      progress =
        "Associated existing installation. Casklet did not install or own it.";
      error = "";
    } catch (cause) {
      error = message(cause);
    } finally {
      installedBusy = false;
      installedLinking = false;
    }
  }
  async function disassociateDetected(item: DetectedApplicationView) {
    if (!selected || installedBusy) return;
    installedBusy = true;
    installedLinking = true;
    try {
      await invoke("disassociate_installed_application", {
        request: {
          version: CONTRACT_VERSION,
          caskId: selected.id,
          installationId: item.installationId,
        },
      });
      await refreshAssociated();
      void refreshAssociationSummary(selected.id);
      progress = "Association removed. The installation is unchanged.";
      error = "";
    } catch (cause) {
      error = message(cause);
    } finally {
      installedBusy = false;
      installedLinking = false;
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
    const includeApps =
      !resourceIds.length &&
      (!removalCategory || removalCategory === "applications") &&
      associatedApps.length > 0;
    if (!scoped.length && !includeApps) {
      removalPlan = null;
      return;
    }
    removalBusy = true;
    historyPlanId = "";
    const previous = removalPlan?.id;
    try {
      const scope = {
        version: CONTRACT_VERSION,
        caskId: selected.id,
        resourceIds,
        categoryKey: resourceIds.length ? null : removalCategory || null,
        permanent: permanentPlan,
      };
      if (
        includeApps ||
        applicationRemovalId ||
        (!resourceIds.length && !removalCategory)
      ) {
        categoryCleanup = await invoke<CategoryCleanupPlan>(
          "create_category_cleanup",
          {
            request: { scope, installationId: applicationRemovalId || null },
          },
        );
        removalPlan = categoryCleanup.resources;
      } else {
        categoryCleanup = null;
        removalPlan = await invoke<RemovalPlanView>("create_removal_plan", {
          request: scope,
        });
      }
      permanentConfirmation = false;
      preserveConfirmed = false;
      reviewedPlanHash = "";
      executionConfirmed = false;
      progress =
        removalPlan?.id === previous
          ? "Current cleanup plan loaded."
          : "Cleanup plan ready.";
      error = "";
      await tick();
      const heading = document.getElementById("cleanup-plan-heading");
      heading?.scrollIntoView?.({ block: "start", behavior: "instant" });
      heading?.focus({ preventScroll: true });
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
    categoryCleanup = null;
    applicationRemovalId = installationId;
    if (installationId) removalCategory = "applications";
    if (cleanupResourceIds.join() !== resourceIds.join()) {
      removalPlan = null;
      reviewedPlanHash = "";
      executionConfirmed = false;
    }
    cleanupResourceIds = resourceIds;
    detailTab = "cleanup";
    if (selected) {
      const pending = await invoke<
        import("../contracts/generated/casklet").MsiCleanupPlan | null
      >("latest_msi_cleanup", {
        request: { version: CONTRACT_VERSION, id: selected.id },
      }).catch(() => null);
      if (pending && ["PARTIAL", "EXECUTING"].includes(pending.state)) {
        categoryCleanup = {
          version: CONTRACT_VERSION,
          caskId: selected.id,
          resources: null,
          applications: pending,
        };
      }
    }
    if (
      selected &&
      !resourceIds.length &&
      !installationId &&
      !removalCategory
    ) {
      const whole = await invoke<CategoryCleanupPlan | null>(
        "pending_whole_cask_cleanup",
        {
          request: { version: CONTRACT_VERSION, id: selected.id },
          resourcePlanId: null,
        },
      ).catch(() => null);
      if (whole?.caskRemovalId) {
        categoryCleanup = whole;
        removalPlan = whole.resources;
      }
    }
    await tick();
    document.getElementById("cleanup-heading")?.focus();
  }
  function invalidateCleanupReview() {
    categoryCleanup = null;
    applicationRemovalId = "";
    removalPlan = null;
    reviewedPlanHash = "";
    executionConfirmed = false;
    permanentConfirmation = false;
  }
  function previewSelectedResource() {
    const id = selectedResource?.id;
    closeResourceOverlay();
    if (id) void navigateCleanup([id]);
  }
  async function reconcileCleanupFailure(cause: unknown) {
    executionConfirmed = false;
    permanentConfirmation = false;
    preserveConfirmed = false;
    if (categoryCleanup) {
      try {
        categoryCleanup = await invoke<CategoryCleanupPlan>(
          "get_category_cleanup",
          { request: categoryCleanup },
        );
        removalPlan = categoryCleanup.resources;
      } catch {
        /* Keep the last inspected snapshots if journal access fails. */
      }
    } else if (removalPlan) {
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
    const review = categoryReview ?? removalPlan;
    if (
      !review ||
      removalBusy ||
      !removalEnabled ||
      reviewedPlanHash !== review.planHash
    )
      return;
    removalBusy = true;
    try {
      if (categoryCleanup) {
        categoryCleanup = await invoke<CategoryCleanupPlan>(
          "approve_category_cleanup",
          { request: categoryCleanup },
        );
        removalPlan = categoryCleanup.resources;
      } else
        removalPlan = await invoke<RemovalPlanView>("approve_removal_plan", {
          request: {
            version: CONTRACT_VERSION,
            id: review.id,
            planHash: review.planHash,
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
    const review = categoryReview ?? removalPlan;
    if (
      !review ||
      removalBusy ||
      !removalEnabled ||
      !executionConfirmed ||
      reviewedPlanHash !== review.planHash ||
      (review.permanent && !permanentConfirmation)
    )
      return;
    removalBusy = true;
    try {
      if (categoryCleanup) {
        categoryCleanup = await invoke<CategoryCleanupPlan>(
          "execute_category_cleanup",
          {
            request: {
              plan: categoryCleanup,
              confirmPermanent: review.permanent && permanentConfirmation,
            },
          },
        );
        removalPlan = categoryCleanup.resources;
        if (categoryCleanup.caskRemoved) {
          if (categoryCleanup.caskRemovalWarning)
            notify(categoryCleanup.caskRemovalWarning, "info");
          removed = true;
          selected = null;
          removalPlan = null;
          categoryCleanup = null;
          progress = "Cask permanently removed.";
          await refresh();
          return;
        }
      } else
        removalPlan = await invoke<RemovalPlanView>("execute_removal_plan", {
          request: {
            version: CONTRACT_VERSION,
            id: review.id,
            planHash: review.planHash,
            confirmPermanent: review.permanent && permanentConfirmation,
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
    if (!preserveConfirmed || removalBusy) return;
    if (
      categoryCleanup?.applications &&
      ["PARTIAL", "EXECUTING"].includes(categoryCleanup.applications.state)
    ) {
      removalBusy = true;
      try {
        const queue = categoryCleanup.applications;
        await invoke("close_msi_cleanup", {
          request: {
            version: CONTRACT_VERSION,
            id: queue.id,
            planHash: queue.planHash,
          },
        });
        categoryCleanup = null;
        removalPlan = null;
        invalidateCleanupReview();
        await refreshResources();
      } catch (cause) {
        error = message(cause);
      } finally {
        removalBusy = false;
      }
      return;
    }
    if (!removalPlan?.runId) return;
    removalBusy = true;
    try {
      removalPlan = await invoke<RemovalPlanView>("close_uncertain_removal", {
        request: {
          version: CONTRACT_VERSION,
          runId: removalPlan.runId,
        },
      });
      if (categoryCleanup)
        categoryCleanup = { ...categoryCleanup, resources: removalPlan };
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
    if (mode === "activity")
      activityCasks = await invoke<CaskView[]>("list_casks", {
        request: { version: CONTRACT_VERSION },
      });
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
      pendingPaths = [];
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
    // History displays one file plan. Do not route its controls to an
    // application queue discovered while navigating to Cleanup.
    categoryCleanup = await invoke<CategoryCleanupPlan | null>(
      "pending_whole_cask_cleanup",
      {
        request: { version: CONTRACT_VERSION, id: plan.caskId },
        resourcePlanId: plan.id,
      },
    ).catch(() => null);
    if (!categoryCleanup?.caskRemovalId) categoryCleanup = null;
    applicationRemovalId = "";
    removalPlan = categoryCleanup?.resources ?? plan;
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
  let folderContents = $state<ResourceContents | null>(null);
  let syncingFiles = false;
  async function inspectFolder(resource: ResourceView, refresh = true) {
    if (refresh) folderContents = null;
    if (resource.kind !== "DIRECTORY") return;
    try {
      const value = await invoke<ResourceContents>("resource_contents", {
        request: { version: CONTRACT_VERSION, id: resource.id },
        refresh,
      });
      if (
        selectedResource?.id === resource.id &&
        value?.resourceId === resource.id
      )
        folderContents = value;
    } catch (cause) {
      error = `Folder inspection failed: ${message(cause)}`;
    }
  }
  let measuringSizes = $state(false);
  async function refreshSizes() {
    if (!selected || measuringSizes) return;
    measuringSizes = true;
    const caskId = selected.id;
    ++associationSummaryRevision;
    try {
      const value = await invoke<StorageSummary>("storage_summary", {
        request: { version: CONTRACT_VERSION, caskId },
        refresh: true,
      });
      const current = await invoke<ResourceView[]>("list_resources", {
        request: { version: CONTRACT_VERSION, caskId },
      });
      if (selected?.id !== caskId) return;
      summary = value;
      resources = current;
      if (selectedResource)
        selectedResource =
          current.find((item) => item.id === selectedResource?.id) ?? null;
      progress = "Saved sizes updated.";
    } catch (cause) {
      error = `Size refresh failed: ${message(cause)}`;
    } finally {
      measuringSizes = false;
    }
  }
  async function syncFiles() {
    if (
      !selected ||
      removed ||
      syncingFiles ||
      busy ||
      removalBusy ||
      measuringSizes
    )
      return;
    syncingFiles = true;
    const caskId = selected.id;
    try {
      const current = await invoke<ResourceView[]>("list_resources", {
        request: { version: CONTRACT_VERSION, caskId },
      });
      const sizes = await invoke<StorageSummary>("storage_summary", {
        request: { version: CONTRACT_VERSION, caskId },
      });
      if (selected?.id !== caskId) return;
      resources = current;
      summary = sizes;
      if (selectedResource) {
        selectedResource =
          current.find((resource) => resource.id === selectedResource?.id) ??
          null;
        // Poll saved state only. Folder enumeration is an explicit user inspection.
      }
    } catch (cause) {
      error = `Folder refresh failed: ${message(cause)}`;
    } finally {
      syncingFiles = false;
    }
  }
  async function pick(directory: boolean, reference = false) {
    try {
      const picked = await open({ multiple: true, directory });
      if (picked) {
        pendingPaths = Array.isArray(picked) ? picked : [picked];
        pendingCategory = "auto";
        intakeFailures = [];
        pendingMode = reference ? "reference" : "";
        referencePicker = false;
      }
    } catch (cause) {
      error = message(cause);
    }
  }
  async function addResource() {
    if (
      busy ||
      !selected ||
      !pendingPath ||
      !pendingMode ||
      (pendingMode === "adopt" && !ownershipConfirmed)
    )
      return;
    busy = true;
    error = "";
    try {
      const result = await addResourceBatch({
        paths: [...pendingPaths],
        caskId: selected.id,
        owned: pendingMode === "adopt",
        categoryKey: pendingCategory,
        existingIds: resources.map((item) => item.id),
        onProgress: (done, total) => {
          batchProgress = `Adding ${done} of ${total} items…`;
        },
      });
      intakeFailures = result.failures;
      pendingPaths = result.failures.map((item) => item.path);
      const complete = result.failures.length === 0;
      if (complete) {
        pendingMode = "";
        ownershipConfirmed = false;
        adding = false;
      }
      selectedResource = null;
      intakeResult = `${result.added} added · ${result.skipped} already included or duplicate · ${result.failures.length} failed.`;
      progress = result.failures.length ? "" : intakeResult;
      if (result.failures.length)
        notify(
          "Some items need attention. Successful items are already in this Cask.",
          "warning",
          "workspace-status",
        );
      await refreshResources();
    } catch (cause) {
      error = message(cause);
    } finally {
      batchProgress = "";
      await refreshRecovery().catch(() => {});
      await refreshOperations().catch(() => {});
      activeOperation = "";
      busy = false;
    }
  }
  async function createCategory() {
    if (!selected || !categoryName.trim() || categoryCreating) return;
    categoryCreating = true;
    categoryNotice = "";
    categoryError = "";
    try {
      const category = await invoke<CategoryView>("create_category", {
        request: {
          version: CONTRACT_VERSION,
          caskId: selected.id,
          name: categoryName,
        },
      });
      categories = [...categories, category];
      categoryNotice = `Category created: ${category.name}.`;
      categoryName = "";
      pendingCategory = category.key;
    } catch (cause) {
      categoryError = message(cause);
    } finally {
      categoryCreating = false;
    }
  }
  async function changeOwnership() {
    if (
      !selectedResource ||
      !selected ||
      busy ||
      removalBusy ||
      (ownershipTarget && !policyConfirmed) ||
      (!ownershipTarget && selectedResource.ownership !== "OWNED")
    )
      return;
    busy = true;
    error = "";
    try {
      selectedResource = await invoke<ResourceView>("set_resource_ownership", {
        request: {
          version: CONTRACT_VERSION,
          caskId: selected.id,
          resourceId: selectedResource.id,
          expectedOwnership: selectedResource.ownership,
          owned: ownershipTarget,
          confirmOwnership: policyConfirmed,
        },
      });
      ownershipEditor = false;
      policyConfirmed = false;
      await refreshResources();
      if (selectedResource?.kind === "DIRECTORY")
        await inspectFolder(selectedResource);
      progress = "Ownership updated. File contents and location are unchanged.";
    } catch (cause) {
      error = message(cause);
    } finally {
      busy = false;
    }
  }
  async function recategorize(resource: ResourceView, categoryKey: string) {
    if (categoryBusy || categoryKey === resource.categoryKey) return;
    categoryBusy = true;
    error = "";
    try {
      const updated = await invoke<ResourceView>("categorize_resource", {
        request: {
          version: CONTRACT_VERSION,
          resourceId: resource.id,
          categoryKey,
        },
      });
      await refreshResources();
      if (selectedResource?.id === resource.id) {
        selectedResource = updated;
        categoryEditor = false;
      }
      progress = "Category updated.";
    } catch (cause) {
      error = message(cause);
    } finally {
      categoryBusy = false;
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
      void receiveExplorer();
      void syncFiles();
      void refreshRecovery().catch(() => {});
      void refreshOperations().catch(() => {});
      void refreshWingetOperation().catch(() => {});
      void refreshInstallerOperation().catch(() => {});
      if (categoryCleanup && removalBusy) {
        const snapshot = categoryCleanup;
        void invoke<CategoryCleanupPlan>("get_category_cleanup", {
          request: snapshot,
        })
          .then((value) => {
            if (
              removalBusy &&
              categoryCleanup?.applications?.id === snapshot.applications?.id
            )
              categoryCleanup = value;
          })
          .catch(() => {});
      }
    }, 3000);
    window.addEventListener("blur", clearDragFeedback);
    const reconcileOnFocus = () => {
      void syncFiles();
    };
    window.addEventListener("focus", reconcileOnFocus);
    window.addEventListener("keydown", cancelDragFeedback);
    void getCurrentWindow()
      .onDragDropEvent((event) => {
        if (event.payload.type === "enter") {
          dragActive =
            mode === "detail" &&
            selected !== null &&
            !removed &&
            event.payload.paths.length > 0 &&
            !busy &&
            !removalBusy &&
            !installerBlocksMutation &&
            !pendingPath;
          return;
        }
        if (event.payload.type === "over") return;
        clearDragFeedback();
        if (event.payload.type === "drop" && event.payload.paths.length > 0) {
          reviewDroppedPaths(event.payload.paths);
        }
      })
      .then((unlisten) => {
        if (disposed) unlisten();
        else unlistenDrop = unlisten;
      })
      .catch((cause) => {
        error = message(cause);
      });
    void listen<[number, number]>("resource-progress", (event) => {
      const [done, total] = event.payload;
      progress =
        total === 0
          ? `${done} entries inspected`
          : `${done} of ${total} bytes processed`;
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenProgress = unlisten;
    });
    void listen<string>("operation-started", (event) => {
      activeOperation = event.payload;
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenStarted = unlisten;
    });
    void listen<[number, number]>("portable-progress", (event) => {
      const [done, total] = event.payload;
      progress =
        total === 0
          ? `${done} entries inspected`
          : `${done} of ${total} bytes processed`;
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenPortableProgress = unlisten;
    });
    void listen<string>("winget-operation-started", (event) => {
      wingetOperationId = event.payload;
      void refreshWingetOperation();
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenWinget = unlisten;
    });
    void listen<string>("installer-operation-started", (event) => {
      installerOperationId = event.payload;
      void refreshInstallerOperation();
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlistenInstaller = unlisten;
    });
    return () => {
      disposed = true;
      window.clearInterval(poll);
      clearDragFeedback();
      window.removeEventListener("blur", clearDragFeedback);
      window.removeEventListener("focus", reconcileOnFocus);
      window.removeEventListener("keydown", cancelDragFeedback);
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
      void receiveExplorer();
      await tick();
      onReady();
    }
  }
  let openingLocation = $state(false);
  async function openResourceLocation(id: string) {
    if (openingLocation) return;
    openingLocation = true;
    try {
      await invoke("open_resource_location", {
        request: { version: CONTRACT_VERSION, id },
      });
    } catch (cause) {
      error = `Unable to open location: ${message(cause)}`;
    } finally {
      openingLocation = false;
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
    if (!selected || metadataBusy || !editName.trim() || !settingsChanged)
      return;
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
  function closeResourceIntake() {
    adding = false;
    pendingPaths = [];
    pendingMode = "";
    ownershipConfirmed = false;
    referencePicker = false;
  }
</script>

<svelte:window onkeydown={cancelManagementEditor} />

<div
  class:detail-window={mode === "detail"}
  class:drag-active={dragActive}
  class:task-focused={(adding && detailTab === "files") ||
    detailTab === "cleanup"}
  class="workspace"
>
  {#if dragActive}
    <div class="cask-drop-hint" role="status">
      <Icon name="folder" size={36} />
      <span class="cask-drop-title"
        >Drop a file or folder into {selected?.name}</span
      >
      <span class="cask-drop-description"
        >Review how to add it after dropping.</span
      >
    </div>
  {/if}
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
        <span class="workspace-location">Cask workspace</span>
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
      {#each [{ key: "overview", title: "Overview" }, { key: "files", title: "Files & Folders" }, { key: "apps", title: "Applications" }, { key: "settings", title: "Settings" }] as tab (tab.key)}
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
  <div class="workspace-content" bind:this={contentElement}>
    {#if waitingExplorer}
      <section class="review" aria-label="Waiting Explorer item">
        <p>
          An Explorer item is waiting. Finish or close the current review to
          continue.
        </p>
        <p class="path">{waitingExplorer.path}</p>
        <button
          onclick={() => {
            if (waitingExplorer)
              void invoke("explorer_ack", { id: waitingExplorer.id })
                .then(() => {
                  waitingExplorer = null;
                })
                .catch((cause) => {
                  error = message(cause);
                });
          }}>Cancel waiting item</button
        >
      </section>
    {/if}
    {#if explorerInstaller}
      <section class="review" aria-label="Installer action">
        <h2>What would you like to do?</h2>
        <p class="path">{explorerInstaller}</p>
        <p>
          Install the application, or manage just the installer file in {selected?.name}.
        </p>
        <div class="form-row">
          <button class="primary" onclick={() => void installExplorerItem()}
            >Install Application</button
          >
          <button
            onclick={() => {
              const path = explorerInstaller;
              explorerInstaller = "";
              reviewDroppedPaths([path]);
            }}>Add Installer to Cask</button
          >
          <button
            onclick={() => {
              explorerInstaller = "";
            }}>Cancel</button
          >
        </div>
      </section>
    {/if}
    {#if removed}<p role="status">Cask removed permanently.</p>
      <button type="button" onclick={() => void getCurrentWindow().close()}
        >Return to Casks</button
      >{/if}
    {#if loading}<p class="loading" role="status">Loading workspace…</p>{/if}
    {#if selected && detailTab === "overview"}
      <section class="storage-panel" aria-label="Storage summary">
        <div class="section-heading">
          <div>
            <h2>Storage</h2>
            <p class="storage-caption">Last known size, including references</p>
          </div>
          <strong class="storage-total">{bytes(totalKnownBytes)}</strong>
        </div>
        <div
          class="storage-bar"
          role="group"
          aria-label="Known logical storage breakdown"
        >
          <span
            class="storage-files"
            style:width={`${totalKnownBytes ? (managedFileBytes / totalKnownBytes) * 100 : 0}%`}
          ></span><span
            class="storage-apps"
            style:width={`${totalKnownBytes ? ((portableBytes + (summary?.ownedApplicationBytes ?? 0)) / totalKnownBytes) * 100 : 0}%`}
          ></span><span
            class="storage-refs"
            style:width={`${totalKnownBytes ? (((summary?.referencedLogicalBytes ?? 0) + (summary?.referenceApplicationBytes ?? 0)) / totalKnownBytes) * 100 : 0}%`}
          ></span>
        </div>
        <div class="storage-legend">
          <div>
            <span class="swatch files"></span>Managed files & folders<strong
              >{bytes(managedFileBytes)}</strong
            >
          </div>
          <div>
            <span class="swatch apps"></span>Owned applications<strong
              >{bytes(
                portableBytes + (summary?.ownedApplicationBytes ?? 0),
              )}</strong
            >
          </div>
          <div>
            <span class="swatch refs"></span>References<strong
              >{bytes(
                (summary?.referencedLogicalBytes ?? 0) +
                  (summary?.referenceApplicationBytes ?? 0),
              )}</strong
            >
          </div>
        </div>
        <button
          class="text-button"
          type="button"
          disabled={measuringSizes}
          onclick={() => void refreshSizes()}
        >
          <Icon name="refresh" size={14} />{measuringSizes
            ? "Measuring sizes…"
            : "Refresh sizes"}
        </button>
        <details
          class="storage-measurement category-storage"
          ontoggle={(event) => {
            if (event.currentTarget.open)
              event.currentTarget
                .closest("section")
                ?.scrollIntoView?.({ block: "start" });
          }}
        >
          <summary
            >Storage by category <span class="count"
              >{categoryGroups.length}</span
            ></summary
          >
          {#if categoryGroups.length}<dl>
              {#each categoryGroups as group (group.key)}<div>
                  <dt>
                    <span class="category-storage-identity"
                      ><span class="category-marker"
                        ><Icon name={categoryIcon(group.key)} size={18} /></span
                      ><span
                        >{group.name}<small
                          >{group.items.length}
                          {group.items.length === 1 ? "item" : "items"}</small
                        ></span
                      ></span
                    >
                  </dt>
                  <dd>{groupSize(group)}</dd>
                </div>{/each}
            </dl>
            <p class="footnote">
              Files and folders only; application sizes are included in the
              overall total above. Nested items are included in their containing
              folder. Refresh sizes to measure unknown content.
            </p>
          {:else}<p class="footnote">
              Add files or folders to see storage by category.
            </p>{/if}
        </details>
      </section>
      <div class="compact-stats" aria-label="Cask contents">
        <div>
          <span
            >Total Items<strong>{fileResources.length + appCount}</strong></span
          >
        </div>
        <button
          type="button"
          aria-label="Browse files and folders"
          onclick={() => {
            detailTab = "files";
          }}
        >
          <Icon name="folder" size={20} /><span
            >Files & Folders<strong>{fileResources.length}</strong></span
          ><Icon name="forward" size={15} />
        </button>
        <button
          type="button"
          aria-label="Browse applications"
          onclick={() => {
            detailTab = "apps";
          }}
        >
          <Icon name="package" size={20} /><span
            >Applications<strong>{appCount}</strong></span
          ><Icon name="forward" size={15} />
        </button>
      </div>
      <section class="information-panel" aria-label="Cask information">
        <dl class="cask-dates">
          <div>
            <dt><Icon name="clock" size={14} />Created</dt>
            <dd>
              <time datetime={new Date(selected.createdAtMs).toISOString()}
                >{new Date(selected.createdAtMs).toLocaleDateString(undefined, {
                  month: "short",
                  day: "numeric",
                  year: "numeric",
                })}</time
              >
            </dd>
          </div>
          <div>
            <dt><Icon name="clock" size={14} />Last Updated</dt>
            <dd>
              <time
                datetime={new Date(selected.updatedAtMs).toISOString()}
                title={new Date(selected.updatedAtMs).toLocaleString()}
                >{activityDay(selected.updatedAtMs)}</time
              >
            </dd>
          </div>
        </dl>
      </section>
    {/if}
    {#if mode === "activity"}
      <section id="operations" aria-label="Operation center">
        <div class="section-heading">
          <h2>Recent activity</h2>
          <button
            class="secondary-button"
            type="button"
            onclick={clearActivity}
            disabled={!recentOperations.some((item) =>
              [
                "COMPLETE",
                "SUCCEEDED",
                "CANCELLED",
                "FAILED",
                "CLOSED_PRESERVED",
                "CONFIRMED_ASSOCIATION",
              ].includes(item.state),
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
                  name={[
                    "COMPLETE",
                    "SUCCEEDED",
                    "CONFIRMED_ASSOCIATION",
                  ].includes(operation.state)
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
                  <span class="activity-state"
                    >{label(operation.state)}<span class="activity-context">
                      · {activityCasks.find(
                        (cask) => cask.id === operation.caskId,
                      )?.name ?? "Removed Cask"}</span
                    ></span
                  >
                  {#if operation.state === "RUNNING"}<p
                      class="activity-progress"
                    >
                      {operation.progressCompleted}{operation.progressTotal ===
                      null
                        ? " entries processed"
                        : ` of ${operation.progressTotal} processed`}
                    </p>{/if}
                  <div class="activity-actions">
                    {#if operation.step === "INSTALL_APPLICATION" && ["REVIEW_REQUIRED", "RECOVERY_REQUIRED"].includes(operation.state)}
                      <button
                        class="text-button"
                        type="button"
                        onclick={() =>
                          void invoke("open_cask_window", {
                            request: {
                              version: CONTRACT_VERSION,
                              id: operation.caskId,
                            },
                          })}>View installation</button
                      >
                    {/if}
                    <details class="metadata-disclosure">
                      <summary>Details</summary>
                      <p class="activity-cask">
                        <strong>Cask:</strong>
                        {activityCasks.find(
                          (cask) => cask.id === operation.caskId,
                        )?.name ?? "Removed Cask"}
                      </p>
                      <p class="activity-description">
                        {activityDescription(operation)}
                      </p>
                    </details>
                    {#if operation.state === "RUNNING" && operation.step !== "INSTALL_APPLICATION"}<button
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
              disabled={installerBusy || installerBlocksMutation}
              onclick={() => void inspectInstaller()}
            >
              {installerBusy
                ? "Working…"
                : installerPreview
                  ? "Choose another installer"
                  : "Select MSI or EXE"}
            </button>
            {#if installerSourceWarning}<div
                class="installer-source-warning"
                role="alert"
              >
                <p>{installerSourceWarning}</p>
                <button
                  type="button"
                  onclick={() => {
                    applicationWorkflow = "existing";
                    installerSourceWarning = "";
                    void focusControl("installed-search");
                  }}>Find installed application</button
                >
              </div>{/if}
            {#if installerPreview && !installerBlocksMutation}
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
              {#if ["REVIEW_REQUIRED", "RECOVERY_REQUIRED"].includes(installerOperation.state)}
                <div class="installer-recovery">
                  <h4>Resolve this installation attempt</h4>
                  <p>
                    Close the installer or application and any processes it
                    opened before continuing. Closing this attempt keeps files
                    and software in place and releases the installation lock. It
                    does not uninstall or undo changes.
                  </p>
                  <label class="checkbox-label"
                    ><input
                      type="checkbox"
                      bind:checked={installerRecoveryConfirmed}
                      disabled={installerBusy}
                    />The installer or application and any processes it opened
                    have finished.</label
                  >
                  <button
                    type="button"
                    disabled={installerBusy || !installerRecoveryConfirmed}
                    onclick={() => void closeInstallerAttempt()}
                    >{installerBusy
                      ? "Closing…"
                      : "Close attempt and keep files"}</button
                  >
                </div>
              {/if}
              {#if installerOperation.state === "RUNNING"}<button
                  type="button"
                  onclick={() => void cancelInstaller()}
                  >Cancel installation</button
                >
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
                  >{installedBusy && !installedLinking
                    ? "Searching…"
                    : "Search"}</button
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
            {#if installedBusy && !installedLinking}<p
                class="loading"
                role="status"
              >
                Reading installed applications…
              </p>{:else if detectedApps.length}
              <ul class="application-list picker-results">
                {#each detectedApps as item (item.installationId)}
                  <li>
                    <ApplicationIcon
                      source={{ kind: "registry", key: item.sourceKey }}
                    />
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
              onclick={closeResourceIntake}
              ><Icon name="close" size={16} /></button
            >
          </div>
          {#if !pendingPath}<div class="intake-welcome">
              <span class="intake-welcome-icon"
                ><Icon name="folder" size={30} /></span
              >
              <h2>Bring this activity together</h2>
              <p>
                Choose files or folders, or drop a selection into this window.
                You’ll review how it belongs to this Cask next.
              </p>
            </div>
            <div class="resource-source-actions">
              <button
                type="button"
                aria-label="Add File"
                onclick={() => void pick(false)}
                ><Icon name="file" size={22} /><span
                  ><strong>Add Files</strong><small
                    >Choose a document or any file</small
                  ></span
                ><Icon name="forward" size={16} /></button
              ><button
                type="button"
                aria-label="Add Folder"
                onclick={() => void pick(true)}
                ><Icon name="folder" size={22} /><span
                  ><strong>Add Folders</strong><small
                    >Keep a folder and its contents together</small
                  ></span
                ><Icon name="forward" size={16} /></button
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
                  >{pendingPaths.length > 1
                    ? `${pendingPaths.length} selected items`
                    : pendingPath.split(/[\\/]/).pop()}</strong
                ><span class="truncate path" title={pendingPath}
                  >{pendingPath}</span
                >
              </div>
              <button
                class="text-button"
                type="button"
                disabled={busy}
                onclick={() => {
                  pendingPaths = [];
                  pendingMode = "";
                  ownershipConfirmed = false;
                }}>Change</button
              >
            </div>
            {#if pendingPaths.length > 1}<details class="intake-selection">
                <summary>Selected items ({pendingPaths.length})</summary>
                <ul>
                  {#each pendingPaths as path, index (index)}<li class="path">
                      {path}
                    </li>{/each}
                </ul>
              </details>{/if}
            {#if intakeFailures.length}<div
                role="alert"
                class="intake-failures"
              >
                <strong>Some items could not be added</strong>
                <p role="status">{intakeResult}</p>
                <ul>
                  {#each intakeFailures as failure, index (index)}<li>
                      <strong>{failure.path.split(/[\\/]/).pop()}</strong>
                      <p>{failure.reason}</p>
                    </li>{/each}
                </ul>
                <p>
                  Successful items are already in this Cask. Retry adds only the
                  remaining selection.
                </p>
              </div>{/if}
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
                    >Keep it here. Allow removal only through reviewed cleanup.</small
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
                    >Keep it here. Cleanup removes the link and preserves the
                    data.</small
                  ></span
                ></label
              >
            </fieldset>
            <div class="intake-category">
              <label for="intake-category">Category</label><select
                id="intake-category"
                bind:value={pendingCategory}
                disabled={busy}
                ><option value="auto">Automatic (file type)</option
                >{#each categories as category (category.key)}<option
                    value={category.key}>{category.name}</option
                  >{/each}</select
              >
            </div>
            {#if pendingMode === "adopt"}<label class="checkbox-label"
                ><input type="checkbox" bind:checked={ownershipConfirmed} /> This
                data is dedicated to this Cask, is not needed elsewhere, and may
                be moved to the Recycle Bin after I approve a cleanup plan.</label
              >{/if}
            <div class="intake-actions">
              {#if busy && batchProgress}<p role="status">
                  {batchProgress}
                </p>{/if}
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
      {#if !adding && !pendingPath}<section aria-label="Resource inventory">
          <div class="section-heading">
            <h2>Files & folders</h2>
            <span class="count">{fileResources.length}</span>
          </div>
          <p class="intro">
            Saved status and sizes. Select an item to open its details; refresh
            sizes in Overview to check for changes.
          </p>
          {#if fileResources.length === 0}<div class="empty-state compact">
              <Icon name="folder" size={28} />
              <h3>No files or folders yet</h3>
              <p>Use Add Items below, or drop an item into this window.</p>
            </div>{/if}
          {#each categoryGroups as group (group.key)}
            <details class="resource-category">
              <summary
                ><span class="category-marker"
                  ><Icon name={categoryIcon(group.key)} size={18} /></span
                ><span class="resource-category-name"
                  >{group.name}<span class="count">{group.items.length}</span
                  ></span
                ><span class="resource-category-size">{groupSize(group)}</span
                ><span class="category-chevron"
                  ><Icon name="down" size={16} /></span
                ></summary
              >
              <ul class="resource-list">
                {#each group.items as resource (resource.id)}<li
                    class="resource-entry"
                  >
                    <button
                      class="resource-toggle"
                      id={`resource-toggle-${resource.id}`}
                      type="button"
                      aria-label={`Details for ${resource.name}`}
                      aria-haspopup="dialog"
                      aria-controls={`resource-details-${resource.id}`}
                      onclick={() => {
                        categoryEditor = false;
                        ownershipEditor = false;
                        policyConfirmed = false;
                        overlayError = "";
                        overlayStatus = "";
                        selectedResource =
                          selectedResource?.id === resource.id
                            ? null
                            : resource;
                        if (selectedResource) {
                          void inspectFolder(selectedResource);
                        } else folderContents = null;
                      }}
                    >
                      <Icon
                        name={categoryIcon(resource.categoryKey, resource.kind)}
                      />
                      <span class="resource-row-copy">
                        <strong title={resource.name}>{resource.name}</strong>
                        <small
                          >{resource.kind === "DIRECTORY"
                            ? "Folder · "
                            : ""}{resource.shared
                            ? "Shared · "
                            : ""}{resource.management === "REFERENCED"
                            ? "Reference"
                            : label(resource.management)} · {categories.find(
                            (category) => category.key === resource.categoryKey,
                          )?.name ?? label(resource.categoryKey)}</small
                        >
                      </span>
                      {#if resource.availability !== "PRESENT"}<span
                          class="resource-state warning"
                          >{label(resource.availability)}</span
                        >{/if}
                      <span class="resource-size"
                        >{bytes(resource.sizeBytes)}</span
                      >
                      <span class="resource-chevron"
                        ><Icon name="forward" size={16} /></span
                      >
                    </button>
                  </li>{/each}
              </ul>
            </details>{/each}
          {#if selectedResource}<dialog
              class="resource-overlay"
              aria-label={`Management details for ${selectedResource.name}`}
              use:mountResourceOverlay
              oncancel={cancelResourceOverlay}
            >
              <div
                class="resource-inspector resource-details"
                id={`resource-details-${selectedResource.id}`}
                role="region"
                aria-label={`Details for ${selectedResource.name}`}
              >
                <div class="inspector-heading">
                  <div>
                    <p class="eyebrow">Management details</p>
                    <h2>{selectedResource.name}</h2>
                  </div>
                  <button
                    class="icon-button"
                    type="button"
                    aria-label="Close resource details"
                    disabled={categoryBusy || (ownershipEditor && busy)}
                    onclick={closeResourceOverlay}
                    ><Icon name="close" size={16} /></button
                  >
                </div>
                {#if overlayError}<p
                    class="overlay-feedback error"
                    role="alert"
                  >
                    {overlayError}
                  </p>{:else if overlayStatus}<p
                    class="overlay-feedback"
                    role="status"
                  >
                    {overlayStatus}
                  </p>{/if}
                <div class="resource-location">
                  <button
                    type="button"
                    disabled={openingLocation}
                    onclick={() => {
                      if (selectedResource)
                        void openResourceLocation(selectedResource.id);
                    }}
                  >
                    <Icon name="folder" size={16} />Open location
                  </button>
                </div>
                <dl class="resource-facts">
                  <div>
                    <dt>Size</dt>
                    <dd>
                      {bytes(selectedResource.sizeBytes)}<small
                        title="File sizes can differ from physical disk usage."
                        >{selectedResource.sizeMeasurement === "LOGICAL"
                          ? "Logical size"
                          : label(selectedResource.sizeMeasurement)}</small
                      >
                    </dd>
                  </div>
                  <div>
                    <dt>Availability</dt>
                    <dd>{label(selectedResource.availability)}</dd>
                  </div>
                  <div>
                    <dt>Management</dt>
                    <dd>
                      {selectedResource.management === "MANAGED" &&
                      selectedResource.containment === "EXTERNAL"
                        ? "Managed in place"
                        : label(selectedResource.management)}
                    </dd>
                  </div>
                </dl>
                <div class="management-setting">
                  <div class="management-setting-row">
                    <span class="setting-symbol"
                      ><Icon name="casks" size={18} /></span
                    >
                    <div>
                      <h4>Category</h4>
                      <p>
                        {categories.find(
                          (category) =>
                            category.key === selectedResource?.categoryKey,
                        )?.name ?? label(selectedResource.categoryKey)}
                      </p>
                    </div>
                    <button
                      id="change-category"
                      type="button"
                      class="setting-edit"
                      aria-expanded={categoryEditor}
                      aria-controls="category-editor"
                      disabled={busy ||
                        removalBusy ||
                        categoryBusy ||
                        selected?.state !== "ACTIVE"}
                      onclick={() => {
                        categoryChoice = selectedResource?.categoryKey ?? "";
                        categoryEditor = !categoryEditor;
                        ownershipEditor = false;
                        if (categoryEditor)
                          void focusControl("category-choice");
                      }}>Change Category</button
                    >
                  </div>
                  {#if categoryEditor}<form
                      id="category-editor"
                      class="management-editor"
                      aria-label="Change item category"
                      onsubmit={(event) => {
                        event.preventDefault();
                        if (selectedResource)
                          void recategorize(selectedResource, categoryChoice);
                      }}
                    >
                      <p class="editor-target">
                        Editing <strong>{selectedResource.name}</strong>
                      </p>
                      <label for="category-choice">Category</label>
                      <div class="select-control">
                        <select
                          id="category-choice"
                          bind:value={categoryChoice}
                          disabled={categoryBusy}
                          >{#each categories as category (category.key)}<option
                              value={category.key}>{category.name}</option
                            >{/each}</select
                        ><Icon name="down" size={16} />
                      </div>
                      <p>
                        Organize this item within the Cask. Its location and
                        ownership stay the same.
                      </p>
                    </form>{/if}
                </div>
                <div class="management-setting">
                  <div class="management-setting-row">
                    <span class="setting-symbol"
                      ><Icon name="link" size={18} /></span
                    >
                    <div>
                      <h4>Ownership</h4>
                      <p>
                        {selectedResource.ownership === "OWNED"
                          ? "Owned by Cask"
                          : selectedResource.management === "REFERENCED"
                            ? "Reference"
                            : label(selectedResource.management)}
                      </p>
                    </div>
                    <button
                      id="change-ownership"
                      type="button"
                      class="setting-edit"
                      aria-expanded={ownershipEditor}
                      aria-controls="ownership-editor"
                      disabled={busy ||
                        removalBusy ||
                        selected?.state !== "ACTIVE"}
                      onclick={() => {
                        categoryEditor = false;
                        ownershipEditor = !ownershipEditor;
                        ownershipTarget =
                          selectedResource?.ownership === "OWNED";
                        policyConfirmed = false;
                        if (ownershipEditor)
                          void focusControl(
                            ownershipTarget
                              ? "ownership-owned"
                              : "ownership-reference",
                          );
                      }}>Change ownership</button
                    >
                  </div>
                  {#if !ownershipEditor}<p class="management-consequence">
                      {selectedResource.management === "REFERENCED"
                        ? "Cleanup removes the link to this Cask and keeps the data."
                        : "Removal always requires a reviewed cleanup plan and current verification."}
                    </p>{/if}
                  {#if ownershipEditor}<form
                      id="ownership-editor"
                      class="management-editor"
                      aria-label="Change item ownership"
                      onsubmit={(event) => {
                        event.preventDefault();
                        void changeOwnership();
                      }}
                    >
                      <p class="editor-target">
                        Editing <strong>{selectedResource.name}</strong>
                      </p>
                      <fieldset class="ownership-options" disabled={busy}>
                        <legend>Ownership</legend>
                        <label class:chosen={ownershipTarget}
                          ><input
                            id="ownership-owned"
                            type="radio"
                            name="ownership-policy"
                            value={true}
                            bind:group={ownershipTarget}
                            onchange={() => {
                              policyConfirmed = false;
                            }}
                          /><span
                            ><strong>Owned by Cask</strong><small
                              >Eligible for reviewed cleanup after verification.</small
                            ></span
                          ></label
                        >
                        <label class:chosen={!ownershipTarget}
                          ><input
                            id="ownership-reference"
                            type="radio"
                            name="ownership-policy"
                            value={false}
                            bind:group={ownershipTarget}
                            onchange={() => {
                              policyConfirmed = false;
                            }}
                          /><span
                            ><strong>Reference</strong><small
                              >Remove only the link. Keep the original data.</small
                            ></span
                          ></label
                        >
                      </fieldset>
                      <p>
                        Files stay in place. Folder contents inherit the
                        selected policy.
                      </p>
                      {#if ownershipTarget}<label class="policy-consent"
                          ><input
                            type="checkbox"
                            bind:checked={policyConfirmed}
                            disabled={busy}
                          />This data is dedicated to this Cask and may be
                          removed through reviewed cleanup.</label
                        >{/if}
                    </form>{/if}
                  {#if selectedResource.kind === "DIRECTORY" && selectedResource.ownership === "OWNED" && !ownershipEditor}<button
                      class="text-button renewal-action"
                      type="button"
                      disabled={busy ||
                        removalBusy ||
                        selected?.state !== "ACTIVE"}
                      onclick={() => {
                        categoryEditor = false;
                        ownershipEditor = true;
                        ownershipTarget = true;
                        policyConfirmed = false;
                        void focusControl("ownership-owned");
                      }}
                      ><Icon name="refresh" size={14} />Renew folder ownership</button
                    >{/if}
                </div>
                {#if selectedResource.kind === "DIRECTORY"}
                  <div class="folder-contents">
                    <div class="section-heading">
                      <h4>Folder contents</h4>
                      <button
                        type="button"
                        class="text-button"
                        onclick={() =>
                          void inspectFolder(selectedResource!, true)}
                        >Refresh folder contents</button
                      >
                    </div>
                    <p>
                      {selectedResource.availability === "CHANGED"
                        ? "The registered folder was replaced or cannot be verified; its current contents have no verified cleanup authority."
                        : selectedResource.ownership === "OWNED"
                          ? "Contents inherit this folder’s ownership, including items added later."
                          : "Contents are references; cleanup keeps the external data."}
                    </p>
                    {#if folderContents}
                      <p>
                        {folderContents.totalEntries} entries · {folderContents.unknownEntries
                          ? `${folderContents.unknownEntries} entries could not be inspected`
                          : "Last inspected contents"}
                      </p>
                      <ul>
                        {#each folderContents.entries as entry (entry.relativePath)}
                          <li>
                            <Icon
                              name={entry.kind === "DIRECTORY"
                                ? "folder"
                                : "file"}
                              size={14}
                            /><span>{entry.relativePath}</span><small
                              >{entry.kind === "FILE"
                                ? bytes(entry.sizeBytes)
                                : "Folder"}</small
                            >
                          </li>
                        {/each}
                      </ul>
                      {#if folderContents.totalEntries > folderContents.entries.length}<p
                        >
                          Showing the first {folderContents.entries.length} entries.
                          Storage includes all inspected files.
                        </p>{/if}
                    {:else}<p>Inspecting folder contents…</p>{/if}
                  </div>
                {/if}
                <div class="resource-cleanup-action">
                  <div>
                    <h4>Cleanup this item</h4>
                    <p>Preview what will be removed or kept.</p>
                  </div>
                  <button
                    class="danger-button"
                    type="button"
                    onclick={previewSelectedResource}
                    ><Icon name="trash" size={15} />Cleanup this item</button
                  >
                </div>
              </div>
              {#if categoryEditor || ownershipEditor}<footer
                  class="resource-overlay-actions"
                >
                  {#if categoryEditor}
                    <button
                      type="button"
                      class="secondary-button"
                      disabled={categoryBusy}
                      onclick={() => {
                        categoryEditor = false;
                        void focusControl("change-category");
                      }}>Cancel</button
                    >
                    <button
                      form="category-editor"
                      type="submit"
                      class="primary-button"
                      disabled={categoryBusy ||
                        !categoryChoice ||
                        categoryChoice === selectedResource.categoryKey}
                      >{categoryBusy ? "Saving…" : "Save category"}</button
                    >
                  {:else if ownershipEditor}
                    <button
                      type="button"
                      class="secondary-button"
                      disabled={busy}
                      onclick={() => {
                        ownershipEditor = false;
                        policyConfirmed = false;
                        void focusControl("change-ownership");
                      }}>Cancel</button
                    >
                    <button
                      form="ownership-editor"
                      type="submit"
                      class="primary-button"
                      disabled={busy ||
                        removalBusy ||
                        selected?.state !== "ACTIVE" ||
                        (ownershipTarget && !policyConfirmed) ||
                        (!ownershipTarget &&
                          selectedResource.ownership !== "OWNED")}
                      >{busy ? "Saving…" : "Save ownership"}</button
                    >
                  {/if}
                </footer>{/if}
            </dialog>{/if}
        </section>{/if}
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
          {#if !removalEnabled && (resources.length > 0 || applications.length > 0 || removalPlan || categoryReview)}<p
              class="cleanup-notice"
              id="cleanup-limit"
            >
              <Icon name="info" size={16} /><span
                ><strong>Review only</strong> · Approval and execution are unavailable
                in this build. You can inspect a plan; no cleanup will run.</span
              >
            </p>{/if}
          {#if (caskRemovalStatus?.unfinishedOperations ?? 0) > 0}<p>
              Finish or resolve outstanding work in Activity before removing
              this Cask.
            </p>{/if}
          {#if resources.length > 0 || associatedApps.length > 0}
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
            {#if !applicationRemovalId && resources.some((item) => !removalCategory || item.categoryKey === removalCategory)}
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
            {/if}
            <button
              class="primary-button"
              type="button"
              onclick={() => void previewRemoval()}
              disabled={removalBusy ||
                cleanupPending ||
                selected.state !== "ACTIVE" ||
                ["PARTIAL", "EXECUTING"].includes(removalPlan?.state ?? "")}
              >Create cleanup plan</button
            >
          {:else}<p>
              No items to clean up. Resolve the remaining dependencies before
              removing the Cask.
            </p>{/if}
        {/if}
        {#if categoryReview && !historyPlanId}
          <CleanupReview
            plan={categoryReview}
            removeCask={!!categoryCleanup?.caskRemovalId}
            {removalEnabled}
            {removalBusy}
            {reviewedPlanHash}
            bind:executionConfirmed
            bind:permanentConfirmation
            bind:preserveConfirmed
            onReview={() => {
              reviewedPlanHash = categoryReview!.planHash;
              executionConfirmed = false;
              permanentConfirmation = false;
            }}
            onApprove={() => void approveRemoval()}
            onExecute={() => void executeRemoval()}
            onClose={() => void closeUncertainRemoval()}
          />
        {:else if removalPlan && !historyPlanId}
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
                plan={categoryReview ?? removalPlan}
                removeCask={!!categoryCleanup?.caskRemovalId}
                {removalEnabled}
                {removalBusy}
                {reviewedPlanHash}
                bind:executionConfirmed
                bind:permanentConfirmation
                bind:preserveConfirmed
                onReview={() => {
                  reviewedPlanHash = (categoryReview ?? removalPlan)!.planHash;
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
      <section class="settings-card" aria-label="Cask settings">
        <details class="settings-disclosure">
          <summary aria-label="Cask settings"
            ><div class="settings-heading">
              <span class="setting-symbol"><Icon name="casks" size={20} /></span
              >
              <div>
                <h2>Cask settings</h2>
                <p>The name and purpose of this activity.</p>
              </div>
              <span class="settings-disclosure-chevron"
                ><Icon name="down" size={17} /></span
              >
            </div></summary
          >
          <div class="settings-disclosure-body">
            <form
              id="cask-settings-form"
              class="settings-form"
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
                disabled={metadataBusy}
              />
              <label for="edit-description"
                >Description <span class="optional-label">Optional</span></label
              ><textarea
                id="edit-description"
                bind:value={editDescription}
                maxlength="500"
                rows="3"
                disabled={metadataBusy}
                placeholder="What is this activity for?"
              ></textarea>
              <div class="archive-setting">
                <label class="checkbox-label"
                  ><input
                    type="checkbox"
                    aria-label="Archive this Cask"
                    bind:checked={editArchived}
                    disabled={metadataBusy}
                  /><span
                    ><strong>Archive this Cask</strong><small
                      >Keep its resources and make the Cask read-only. Clear
                      this option to restore it.</small
                    ></span
                  ></label
                >
              </div>
            </form>
          </div>
        </details>
      </section>
      <section class="settings-card" aria-label="Categories">
        <details class="settings-disclosure">
          <summary aria-label="Categories"
            ><div class="settings-heading">
              <span class="setting-symbol"
                ><Icon name="folder" size={20} /></span
              >
              <div>
                <h2>Categories</h2>
                <p>Give resources a role in this Cask.</p>
              </div>
              <span class="settings-disclosure-chevron"
                ><Icon name="down" size={17} /></span
              >
            </div></summary
          >
          <div class="settings-disclosure-body">
            <ul class="category-chips" aria-label="Available categories">
              {#each categories as category (category.key)}<li>
                  {category.name}
                </li>{/each}
            </ul>
            <form
              class="settings-form category-create"
              onsubmit={(event) => {
                event.preventDefault();
                void createCategory();
              }}
            >
              <label for="category-name">New category</label>
              <p class="category-save-note">
                Categories are saved immediately.
              </p>
              <div class="form-row">
                <input
                  id="category-name"
                  bind:value={categoryName}
                  maxlength="80"
                  disabled={categoryCreating}
                  placeholder="e.g. Research"
                /><button
                  class="secondary-button"
                  disabled={categoryCreating || !categoryName.trim()}
                  >{categoryCreating ? "Creating…" : "Create category"}</button
                >
              </div>
            </form>
            <p class="category-feedback" role="status" aria-live="polite">
              {categoryNotice}
            </p>
            {#if categoryError}<p class="category-feedback error" role="alert">
                {categoryError}
              </p>{/if}
          </div>
        </details>
      </section>
      <section class="settings-card" aria-label="Export Cask">
        <details class="settings-disclosure">
          <summary aria-label="Export Cask"
            ><div class="settings-heading">
              <span class="setting-symbol"
                ><Icon name="download" size={20} /></span
              >
              <div>
                <h2>Export Cask</h2>
                <p>Save an inventory of this activity.</p>
              </div>
              <span class="settings-disclosure-chevron"
                ><Icon name="down" size={17} /></span
              >
            </div></summary
          >
          <div class="settings-disclosure-body">
            <div class="settings-action-list">
              <div>
                <div>
                  <h3>Readable report</h3>
                  <p>A Markdown summary for reading or sharing.</p>
                </div>
                <button
                  type="button"
                  onclick={() => void exportCask("markdown")}
                  >Export report<Icon name="download" size={15} /></button
                >
              </div>
              <div>
                <div>
                  <h3>Portable inventory</h3>
                  <p>JSON without machine-specific paths.</p>
                </div>
                <button type="button" onclick={() => void exportCask("json")}
                  >Portable JSON<Icon name="download" size={15} /></button
                >
              </div>
              <details class="settings-advanced">
                <summary>Machine-specific export</summary>
                <p>Includes local paths. Review before sharing.</p>
                <button
                  type="button"
                  onclick={() => void exportCask("json", true)}
                  >Machine JSON<Icon name="download" size={15} /></button
                >
              </details>
            </div>
          </div>
        </details>
      </section>
    {/if}
    {#if mode === "settings"}
      <ThemeSettings />
      <section
        id="settings"
        class="settings-card"
        aria-label="Settings and privacy"
      >
        <div class="settings-heading">
          <span class="setting-symbol"><Icon name="database" size={20} /></span>
          <div>
            <h2>Local data</h2>
            <p>Stored on this computer. No account or default telemetry.</p>
          </div>
        </div>
        <div class="settings-action-list">
          <div>
            <div>
              <h3>Database backup</h3>
              <p>
                Save Cask metadata and associations. Back up your files
                separately.
              </p>
            </div>
            <button
              type="button"
              disabled={backupBusy}
              onclick={() => void backupDatabase()}
              ><Icon name="database" size={16} />{backupBusy
                ? "Backing up…"
                : "Back up database metadata"}</button
            >
          </div>
          <div>
            <div>
              <h3>Diagnostics</h3>
              <p>
                Save a local diagnostic bundle. It may include private paths and
                operation details; review it before sharing.
              </p>
            </div>
            <button type="button" onclick={() => void saveDiagnostics()}
              ><Icon name="download" size={16} />Save diagnostics</button
            >
          </div>
        </div>
      </section>
      <section
        class="settings-card cleanup-capability"
        aria-label="Cleanup availability"
      >
        <div class="settings-heading">
          <span class="setting-symbol"><Icon name="trash" size={20} /></span>
          <div>
            <h2>Cleanup availability</h2>
            <p>
              {removalEnabled
                ? "Available after plan review, approval, and confirmation."
                : "Review only in this build. Destructive cleanup is disabled pending independent security review."}
            </p>
          </div>
        </div>
      </section>
    {/if}
  </div>
  {#if mode === "detail"}<footer class="workspace-footer">
      {#if detailTab === "settings" && selected}
        <button
          type="button"
          class="secondary-button"
          disabled={metadataBusy || !settingsChanged}
          onclick={() => {
            editName = selected!.name;
            editDescription = selected!.description ?? "";
            editArchived = selected!.state === "ARCHIVED";
          }}>Reset changes</button
        >
        <button
          form="cask-settings-form"
          type="submit"
          class="primary-button"
          disabled={metadataBusy || !editName.trim() || !settingsChanged}
          >{metadataBusy ? "Saving…" : "Save changes"}</button
        >
      {:else}
        <button
          class:primary-button={!(adding && detailTab === "files") &&
            ["overview", "files"].includes(detailTab)}
          class:secondary-button={(adding && detailTab === "files") ||
            !["overview", "files"].includes(detailTab)}
          aria-expanded={adding && detailTab === "files"}
          type="button"
          disabled={!selected || busy || selected.state !== "ACTIVE"}
          onclick={() => {
            const nextAdding = !(adding && detailTab === "files");
            detailTab = "files";
            adding = nextAdding;
            if (adding) selectedResource = null;
            else closeResourceIntake();
          }}
          ><Icon name={adding && detailTab === "files" ? "close" : "plus"} />
          {adding && detailTab === "files"
            ? "Close Add Items"
            : "Add Items"}</button
        >
        {#if detailTab !== "cleanup" && !(detailTab === "files" && (adding || pendingPath)) && !(detailTab === "apps" && applicationWorkflow)}
          <button
            type="button"
            disabled={!selected || removalBusy}
            class="danger-button"
            onclick={() => navigateCleanup()}
            ><Icon name="trash" size={17} /> Cleanup</button
          >{/if}
      {/if}
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
