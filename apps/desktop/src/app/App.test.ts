import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/svelte";
import { afterEach, expect, test, vi } from "vitest";
import App from "./App.svelte";
import Workspace from "./Workspace.svelte";
import MsiCleanup from "./MsiCleanup.svelte";
import ToastArea from "./ToastArea.svelte";
import { clearNotices, notify } from "./notifications";
import { revealWindow } from "./windowReady";
import { caskColor } from "./format";
import { applicationRows } from "./applications";
import { open } from "@tauri-apps/plugin-dialog";

vi.stubGlobal("matchMedia", () => ({ matches: true }));
vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) =>
  setTimeout(() => callback(performance.now()), 0),
);

HTMLDialogElement.prototype.showModal = function () {
  this.setAttribute("open", "");
};
HTMLDialogElement.prototype.close = function () {
  this.removeAttribute("open");
};

const calls = vi.hoisted(() => ({
  invoke: vi.fn(),
  nativeTheme: vi.fn<(request: unknown) => Promise<void>>(async () => {}),
  events: new Map<string, (event: { payload: unknown }) => void>(),
  drop: null as
    | null
    | ((event: { payload: { type: string; paths: string[] } }) => void),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, ...args: unknown[]) =>
    command === "set_window_theme"
      ? calls.nativeTheme(args[0])
      : calls.invoke(command, ...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(async () => {}),
  listen: vi.fn(
    async (name: string, handler: (event: { payload: unknown }) => void) => {
      calls.events.set(name, handler);
      return () => {
        calls.events.delete(name);
      };
    },
  ),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    title: async () => "Casklet",
    minimize: vi.fn(),
    close: vi.fn(),
    onDragDropEvent: async (handler: typeof calls.drop) => {
      calls.drop = handler;
      return () => {
        calls.drop = null;
      };
    },
  }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));

afterEach(() => {
  cleanup();
  localStorage.clear();
  calls.nativeTheme.mockClear();
  calls.events.clear();
  clearNotices();
  vi.useRealTimers();
  vi.mocked(open).mockReset();
  calls.invoke.mockReset();
  calls.drop = null;
});

function ownershipWorkspace(
  resources: object[] = [],
  handler?: (command: string) => unknown,
) {
  const cask = {
    version: 1,
    id: "ownership-cask",
    name: "Ownership",
    description: "",
    state: "ACTIVE",
    managedStorageRoot: "C:/Casklets/test",
    createdAtMs: 1,
    updatedAtMs: 1,
    revision: 0,
  };
  calls.invoke.mockImplementation(async (command: string) => {
    const handled = handler?.(command);
    if (handled !== undefined) return handled;
    if (command === "cask_removal_status")
      return {
        version: 1,
        canRemove: resources.length === 0,
        resourceCount: resources.length,
        applicationCount: 0,
        unfinishedOperations: 0,
      };
    if (command === "get_cask") return cask;
    if (command === "list_resources") return resources;
    if (command === "list_categories")
      return [{ version: 1, key: "other", name: "Other", isBuiltin: true }];
    if (command === "list_casks") return [cask];
    if (command === "removal_capability") return false;
    if (command === "recovery_status")
      return { version: 1, operations: [], pendingManifestCasks: [] };
    if (command === "storage_summary")
      return {
        managedLogicalBytes: 0,
        referencedLogicalBytes: 0,
        physicalBytes: null,
        reclaimableMinBytes: 0,
        reclaimableMaxBytes: 0,
        unknownEntries: 0,
      };
    return [];
  });
  render(ToastArea);
  render(Workspace, { caskId: cask.id });
  return cask;
}

test("an empty Cask removes after confirmation without creating a cleanup plan", async () => {
  const cask = ownershipWorkspace([], (command) =>
    command === "remove_cask" ? null : undefined,
  );
  await screen.findByRole("heading", { name: "Ownership" });
  expect(screen.queryByRole("button", { name: "Remove Cask" })).toBeNull();
  await fireEvent.click(screen.getByRole("button", { name: "Cleanup" }));
  const remove = await screen.findByRole("button", { name: "Remove Cask" });
  await waitFor(() =>
    expect((remove as HTMLButtonElement).disabled).toBe(false),
  );
  await fireEvent.click(remove);
  const modal = screen.getByRole("dialog", {
    name: "Permanently remove Cask?",
  });
  expect(modal.getAttribute("open")).not.toBeNull();
  expect(document.activeElement).toBe(
    within(modal).getByRole("button", { name: "Cancel" }),
  );
  expect(
    calls.invoke.mock.calls.some(([command]) => command === "remove_cask"),
  ).toBe(false);
  await fireEvent.click(
    within(screen.getByRole("dialog")).getByRole("button", {
      name: "Remove Cask",
    }),
  );
  await waitFor(() =>
    expect(calls.invoke).toHaveBeenCalledWith("remove_cask", {
      request: { version: 1, id: cask.id, expectedRevision: 0, confirm: true },
    }),
  );
  expect(
    calls.invoke.mock.calls.some(
      ([command]) => command === "create_removal_plan",
    ),
  ).toBe(false);
  await screen.findByText("Cask removed permanently.");
  expect(screen.queryByRole("button", { name: "Archive Cask" })).toBeNull();
});

test("Overview metadata and resource choices stay compact", async () => {
  ownershipWorkspace();
  await screen.findByRole("heading", { name: "Ownership" });
  const info = screen.getByLabelText("Cask information") as HTMLDetailsElement;
  expect(info.open).toBe(false);
  expect(within(info).getByText("Storage Location")).toBeTruthy();
  expect(screen.queryByText("Size on Disk")).toBeNull();
  await fireEvent.click(screen.getByRole("button", { name: "Add Items" }));
  expect(screen.getByRole("button", { name: "Add File" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Add Folder" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Add Reference" })).toBeNull();
});

test("Cask removal invalidates the library immediately and older queries cannot resurrect it", async () => {
  const cask = {
    version: 1,
    id: "deleted",
    name: "Deleted",
    description: "",
    state: "ACTIVE",
    managedStorageRoot: "C:/Casklets/deleted",
    createdAtMs: 1,
    updatedAtMs: 1,
    revision: 0,
  };
  const initial = {
    version: 1,
    totalItems: 0,
    managedBytes: 0,
    unknownEntries: 0,
    storageRoot: "C:/Casklets",
    casks: [
      {
        cask,
        itemCount: 0,
        fileCount: 0,
        applicationCount: 0,
        managedBytes: 0,
        referencedBytes: 0,
        unknownEntries: 0,
      },
    ],
  };
  let requests = 0;
  let completeStale: ((value: typeof initial) => void) | undefined;
  calls.invoke.mockImplementation(async (command) => {
    if (command === "cask_library") {
      requests++;
      if (requests === 1) return initial;
      if (requests === 2)
        return new Promise((resolve) => {
          completeStale = resolve;
        });
      return { ...initial, casks: [] };
    }
    if (command === "recovery_status")
      return { version: 1, operations: [], pendingManifestCasks: [] };
    return null;
  });
  render(App);
  await screen.findByRole("button", { name: "Open Deleted" });
  await waitFor(() => expect(calls.events.has("casks.removed")).toBe(true));
  calls.events.get("casks.changed")?.({ payload: null });
  await waitFor(() => expect(completeStale).toBeDefined());
  calls.events.get("casks.removed")?.({ payload: "deleted" });
  await waitFor(() =>
    expect(screen.queryByRole("button", { name: "Open Deleted" })).toBeNull(),
  );
  completeStale?.(initial);
  await waitFor(() =>
    expect(screen.getByRole("heading", { name: "No Casks yet" })).toBeTruthy(),
  );
  expect(screen.queryByRole("button", { name: "Open Deleted" })).toBeNull();
});

test("installer diagnostics and history are absent while EXE limits remain clear", async () => {
  const path = "C:/private/installers/setup.exe";
  ownershipWorkspace([], (command) =>
    command === "inspect_installer_artifact"
      ? {
          version: 1,
          path,
          format: "EXE",
          sha256: "secret-hash",
          sizeBytes: 12,
          signatureStatus: "UNKNOWN",
          signatureDetail: "PowerShell execution-policy detail",
          signer: null,
          productName: "Example setup",
          publisher: null,
          productVersion: null,
          productCode: null,
          warnings: ["Technical diagnostic"],
        }
      : undefined,
  );
  vi.mocked(open).mockResolvedValue(path);
  await screen.findByRole("heading", { name: "Ownership" });
  await fireEvent.click(screen.getByRole("button", { name: "Applications" }));
  await fireEvent.click(
    screen.getByRole("button", { name: "Add Application" }),
  );
  await fireEvent.click(
    screen.getByRole("button", { name: /Install Application/ }),
  );
  await fireEvent.click(
    screen.getByRole("button", { name: "Select MSI or EXE" }),
  );
  await screen.findByText("Example setup");
  expect(
    screen.getByText(/Cleanup uses the verified Windows uninstaller/i),
  ).toBeTruthy();
  expect(screen.queryByText(path)).toBeNull();
  expect(screen.queryByText(/secret-hash/)).toBeNull();
  expect(screen.queryByText(/PowerShell/)).toBeNull();
  expect(screen.queryByText("Technical details")).toBeNull();
  expect(screen.queryByText(/Advanced installer history/i)).toBeNull();
});

test("Select and Install uses one command and automatically refreshes Applications", async () => {
  let installed = false;
  const path = "C:/setup.msi";
  ownershipWorkspace([], (command) => {
    if (command === "inspect_installer_artifact")
      return {
        version: 1,
        path,
        format: "MSI",
        sha256: "selected-hash",
        sizeBytes: 1,
        signatureStatus: "VALID",
        signer: "Publisher",
        productName: "Example MSI",
        productCode: "{12345678-1234-1234-1234-123456789ABC}",
        warnings: [],
      };
    if (command === "install_application") {
      installed = true;
      return {
        version: 1,
        id: "install-op",
        caskId: "ownership-cask",
        state: "CONFIRMED_ASSOCIATION",
        confirmedInstallationId: "new-app",
        exitCode: 0,
        errorCode: null,
        candidates: [],
        warnings: [],
      };
    }
    if (command === "list_associated_applications")
      return installed
        ? [
            {
              version: 1,
              installationId: "new-app",
              applicationId: "app",
              name: "Installed Example",
              availability: "PRESENT",
              state: "PRESENT",
              removalPolicy: "MANAGED_MSI",
              scope: "USER",
              publisher: "Publisher",
              displayVersion: "1",
              caskId: "ownership-cask",
            },
          ]
        : [];
    return undefined;
  });
  vi.mocked(open).mockResolvedValue(path);
  await screen.findByRole("heading", { name: "Ownership" });
  await fireEvent.click(screen.getByRole("button", { name: "Applications" }));
  await fireEvent.click(
    screen.getByRole("button", { name: "Add Application" }),
  );
  await fireEvent.click(
    screen.getByRole("button", { name: /Install Application/ }),
  );
  await fireEvent.click(
    screen.getByRole("button", { name: "Select MSI or EXE" }),
  );
  const install = await screen.findByRole("button", { name: "Install" });
  expect(
    calls.invoke.mock.calls.some(
      ([command]) => command === "install_application",
    ),
  ).toBe(false);
  await fireEvent.click(
    screen.getByRole("checkbox", { name: "Request administrator privileges" }),
  );
  await fireEvent.click(install);
  await screen.findByText("Installed Example");
  expect(calls.invoke).toHaveBeenCalledWith("install_application", {
    request: {
      version: 1,
      caskId: "ownership-cask",
      path,
      expectedSha256: "selected-hash",
      acceptExecutionRisk: true,
      allowElevation: true,
    },
  });
  expect(
    calls.invoke.mock.calls.filter(
      ([command]) => command === "install_application",
    ),
  ).toHaveLength(1);
  expect(
    calls.invoke.mock.calls.some(([command]) =>
      [
        "register_installer_artifact",
        "launch_installer_artifact",
        "confirm_installer_candidate",
        "close_installer_operation",
      ].includes(command),
    ),
  ).toBe(false);
  expect(screen.queryByText("Recovery required")).toBeNull();
  expect(screen.queryByRole("button", { name: "Review launch" })).toBeNull();
});

function appFixture(
  name: string,
  id: string,
  managed = false,
): Parameters<typeof applicationRows>[1][number] {
  return {
    version: 1,
    applicationId: `app-${id}`,
    installationId: id,
    name,
    displayVersion: "1.0",
    publisher: "Example Publisher",
    scope: "USER",
    sourceKey: "registry",
    installLocation: null,
    state: "PRESENT",
    ambiguousName: false,
    associatedCaskId: "ownership-cask",
    provenance: "DETECTED_WINDOWS_REGISTRY",
    removalPolicy: managed ? "MANAGED_MSI" : "DETACH_ONLY",
  };
}

test("application presentation merges provider rows without inferring ownership from names", () => {
  const app = appFixture("Example", "one");
  const winget = {
    version: 1,
    id: "provider",
    caskId: "ownership-cask",
    packageId: "Example.App",
    packageVersion: "1",
    registryInstallationId: "one",
    state: "INSTALLED",
    provenance: "INSTALLED_BY_CASKLET",
    canPlanUninstall: true,
    warnings: [],
  };
  const merged = applicationRows([], [app], [winget]);
  expect(merged).toHaveLength(1);
  expect(merged[0]).toMatchObject({
    name: "Example",
    kind: "winget",
    management: "managed",
  });
  expect(
    applicationRows([], [app, appFixture("Example", "two")], []),
  ).toHaveLength(2);
  expect(applicationRows([], [app], [])[0].management).toBe("associated");
  expect(
    applicationRows(
      [],
      [],
      [
        { ...winget, registryInstallationId: "", id: "a" },
        { ...winget, registryInstallationId: "", id: "b" },
      ],
    ),
  ).toHaveLength(2);
});

test("Applications keeps Add above a large library and supports focused navigation and filters", async () => {
  const apps = Array.from({ length: 10 }, (_, index) =>
    appFixture(`App ${index}`, `id-${index}`, index % 2 === 0),
  );
  ownershipWorkspace([], (command) =>
    command === "list_associated_applications" ? apps : undefined,
  );
  await screen.findByRole("heading", { name: "Ownership" });
  await fireEvent.click(screen.getByRole("button", { name: "Applications" }));
  const add = screen.getByRole("button", { name: "Add Application" });
  const list = await screen.findByRole("list", { name: "Application list" });
  expect(within(list).getAllByRole("listitem")).toHaveLength(10);
  expect(
    add.compareDocumentPosition(list) & Node.DOCUMENT_POSITION_FOLLOWING,
  ).not.toBe(0);
  await fireEvent.input(
    screen.getByRole("textbox", { name: "Search Cask applications" }),
    { target: { value: "App 7" } },
  );
  expect(
    within(screen.getByRole("list", { name: "Application list" })).getAllByRole(
      "listitem",
    ),
  ).toHaveLength(1);
  await fireEvent.click(
    within(
      screen.getByRole("group", { name: "Filter applications" }),
    ).getByRole("button", { name: "Managed" }),
  );
  expect(screen.getByText("No matching applications")).toBeTruthy();
  await fireEvent.click(screen.getByRole("button", { name: "Reset filters" }));
  expect(screen.getByText("App 7")).toBeTruthy();
  await fireEvent.click(add);
  const install = screen.getByRole("button", { name: /Install Application/ });
  await waitFor(() => expect(document.activeElement).toBe(install));
  await fireEvent.click(install);
  await waitFor(() =>
    expect(
      document.activeElement?.classList.contains("application-workflow-panel"),
    ).toBe(true),
  );
  const panel = screen.getByRole("region", { name: "Install Application" });
  expect(
    panel.compareDocumentPosition(
      screen.getByRole("list", { name: "Application list" }),
    ) & Node.DOCUMENT_POSITION_FOLLOWING,
  ).not.toBe(0);
  await fireEvent.click(
    screen.getByRole("button", { name: "Close application workflow" }),
  );
  await waitFor(() => expect(document.activeElement).toBe(add));
  expect(
    screen.queryByRole("region", { name: "Install Application" }),
  ).toBeNull();
});

test("interrupted installation remains recoverable without exposing installer diagnostics", async () => {
  let kept = false;
  const operation = {
    version: 1,
    id: "private-operation-id",
    artifactId: "artifact",
    caskId: "ownership-cask",
    state: "RECOVERY_REQUIRED",
    exitCode: null,
    errorCode: "LAUNCH_UNCERTAIN",
    confirmedInstallationId: null,
    candidates: [],
    startedAtMs: 1,
    warnings: ["secret implementation detail"],
  };
  ownershipWorkspace([], (command) => {
    if (command === "list_installer_operations") return kept ? [] : [operation];
    if (command === "get_installer_operation")
      return {
        ...operation,
        state: kept ? "CLOSED_PRESERVED" : operation.state,
      };
    if (command === "close_installer_operation") {
      kept = true;
      return null;
    }
    return undefined;
  });
  await screen.findByRole("heading", { name: "Ownership" });
  await fireEvent.click(screen.getByRole("button", { name: "Applications" }));
  await fireEvent.click(
    await screen.findByRole("button", { name: "View installation" }),
  );
  expect(
    screen.queryByText(
      /private-operation-id|LAUNCH_UNCERTAIN|secret implementation/,
    ),
  ).toBeNull();
  expect(screen.queryByText("Technical details")).toBeNull();
  const keep = screen.getByRole("button", { name: "Keep installed changes" });
  expect((keep as HTMLButtonElement).disabled).toBe(true);
  await fireEvent.click(
    screen.getByRole("checkbox", {
      name: "The installer and any apps it opened have finished.",
    }),
  );
  await fireEvent.click(keep);
  await waitFor(() =>
    expect(calls.invoke).toHaveBeenCalledWith("close_installer_operation", {
      request: { version: 1, id: "private-operation-id" },
    }),
  );
  expect(
    calls.invoke.mock.calls.some(
      ([command]) => command === "install_application",
    ),
  ).toBe(false);
});

test("clearing completed Activity preserves active and recovery work across reload", async () => {
  const entries = ["COMPLETE", "RUNNING", "RECOVERY_REQUIRED"].map(
    (state, index) => ({
      version: 1,
      id: `op-${index}`,
      caskId: "ownership-cask",
      state,
      step: `Step ${index}`,
      updatedAtMs: 1,
      startedAtMs: 1,
      progressCompleted: 0,
      progressTotal: null,
      errorCode: null,
      cancellationRequested: false,
    }),
  );
  ownershipWorkspace([], (command) =>
    command === "list_operations" ? entries : undefined,
  );
  await screen.findByRole("heading", { name: "Ownership" });
  await fireEvent.click(screen.getByRole("button", { name: "Activity" }));
  await screen.findByText("Step 0");
  await fireEvent.click(screen.getByRole("button", { name: "Clear history" }));
  expect(screen.queryByText("Step 0")).toBeNull();
  expect(screen.getByText("Step 1")).toBeTruthy();
  expect(screen.getByText("Step 2")).toBeTruthy();
  expect(
    calls.invoke.mock.calls.some(([command]) =>
      /clear|delete.*operation/.test(command),
    ),
  ).toBe(false);
  cleanup();
  ownershipWorkspace([], (command) =>
    command === "list_operations" ? entries : undefined,
  );
  await screen.findByRole("heading", { name: "Ownership" });
  await fireEvent.click(screen.getByRole("button", { name: "Activity" }));
  await screen.findByText("Step 1");
  expect(screen.queryByText("Step 0")).toBeNull();
});

test("MSI application cleanup separates preview, review, approval and confirmed execution", async () => {
  let plan = {
    version: 1,
    id: "msi-plan",
    caskId: "test",
    state: "DRAFT",
    planHash: "hash",
    expiresAtMs: Date.now() + 600000,
    items: [
      {
        installationId: "msi",
        name: "Managed app",
        productCode: "{PRODUCT}",
        evidenceSha256: "evidence",
        action: "UNINSTALL",
        state: "PENDING",
      },
    ],
  };
  calls.invoke.mockImplementation(async (command) => {
    if (command === "latest_msi_cleanup") return null;
    if (command === "approve_msi_cleanup")
      plan = { ...plan, state: "APPROVED" };
    if (command === "execute_msi_cleanup")
      plan = {
        ...plan,
        state: "COMPLETE",
        items: plan.items.map((item) => ({ ...item, state: "SUCCEEDED" })),
      };
    return plan;
  });
  render(MsiCleanup, {
    caskId: "test",
    enabled: true,
    hasManaged: true,
    onChanged: async () => {},
  });
  await fireEvent.click(
    screen.getByRole("button", { name: "Preview application cleanup" }),
  );
  await screen.findByText("Managed app");
  expect(
    calls.invoke.mock.calls.some(
      ([command]) => command === "execute_msi_cleanup",
    ),
  ).toBe(false);
  await fireEvent.click(
    screen.getByRole("button", { name: "Review applications" }),
  );
  await fireEvent.click(
    screen.getByRole("button", { name: "Approve uninstall" }),
  );
  await waitFor(() =>
    expect(
      (
        screen.getByRole("button", {
          name: "Uninstall applications",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true),
  );
  await fireEvent.click(screen.getByLabelText(/I confirm uninstalling/));
  await fireEvent.click(
    screen.getByRole("button", { name: "Uninstall applications" }),
  );
  await screen.findByText("Uninstalled");
  expect(calls.invoke).toHaveBeenCalledWith("execute_msi_cleanup", {
    request: { version: 1, id: "msi-plan", planHash: "hash" },
  });
});

test("in-place ownership needs explicit consent and uses the shared adoption command", async () => {
  const resources: object[] = [];
  ownershipWorkspace(resources, (command) => {
    if (command !== "adopt_resource") return undefined;
    const resource = {
      id: "owned",
      name: "project.txt",
      caskId: "ownership-cask",
      path: "D:/Projects/project.txt",
      kind: "FILE",
      management: "MANAGED",
      ownership: "OWNED",
      containment: "EXTERNAL",
      availability: "PRESENT",
      categoryKey: "other",
      sizeBytes: 4,
      sizeMeasurement: "EXACT",
      unknownEntries: 0,
    };
    resources.push(resource);
    return resource;
  });
  await screen.findByRole("heading", { name: "Ownership" });
  await waitFor(() => expect(calls.drop).not.toBeNull());
  calls.drop?.({
    payload: { type: "drop", paths: ["D:/Projects/project.txt"] },
  });
  await fireEvent.click(
    await screen.findByLabelText("Manage at current location"),
  );
  const add = screen.getByRole("button", { name: "Add resource" });
  expect((add as HTMLButtonElement).disabled).toBe(true);
  await fireEvent.click(
    screen.getByLabelText(/This data is dedicated to this Cask/),
  );
  await fireEvent.click(add);
  await waitFor(() =>
    expect(calls.invoke).toHaveBeenCalledWith("adopt_resource", {
      request: {
        version: 1,
        caskId: "ownership-cask",
        sourcePath: "D:/Projects/project.txt",
        categoryKey: "other",
        confirmOwnership: true,
      },
    }),
  );
  expect(
    calls.invoke.mock.calls.some(([command]) => command === "add_resource"),
  ).toBe(false);
  const toggle = await screen.findByRole("button", {
    name: "Details for project.txt",
  });
  expect(toggle.getAttribute("aria-expanded")).toBe("false");
  expect(screen.queryByText("D:/Projects/project.txt")).toBeNull();
  await fireEvent.click(toggle);
  const details = screen.getByRole("region", {
    name: "Details for project.txt",
  });
  expect(within(details).getByText("D:/Projects/project.txt")).toBeTruthy();
  expect(within(details).getByText("Cleanup authority")).toBeTruthy();
  await fireEvent.click(toggle);
  expect(
    screen.queryByRole("region", { name: "Details for project.txt" }),
  ).toBeNull();
  expect(screen.queryByText("D:/Projects/project.txt")).toBeNull();
});

test("cleanup review explains actions and blocks concurrent duplicate submissions", async () => {
  const resource = {
    id: "reference",
    name: "keep.txt",
    caskId: "ownership-cask",
    path: "D:/keep.txt",
    kind: "FILE",
    management: "REFERENCED",
    ownership: "REFERENCED",
    containment: "EXTERNAL",
    availability: "PRESENT",
    categoryKey: "other",
    sizeBytes: 4,
    unknownEntries: 0,
  };
  let finish!: (value: unknown) => void;
  const pending = new Promise((resolve) => {
    finish = resolve;
  });
  ownershipWorkspace([resource], (command) =>
    command === "create_removal_plan" ? pending : undefined,
  );
  await screen.findByRole("heading", { name: "Ownership" });
  await fireEvent.click(await screen.findByRole("button", { name: "Cleanup" }));
  expect(
    calls.invoke.mock.calls.some(
      ([command]) => command === "create_removal_plan",
    ),
  ).toBe(false);
  await fireEvent.click(screen.getByRole("button", { name: "Cleanup" }));
  expect(screen.getByText("Review only")).toBeTruthy();
  const create = await screen.findByRole("button", {
    name: "Create cleanup plan",
  });
  await fireEvent.click(create);
  await fireEvent.click(create);
  expect(
    calls.invoke.mock.calls.filter(
      ([command]) => command === "create_removal_plan",
    ),
  ).toHaveLength(1);
  finish({
    version: 1,
    id: "stable-plan",
    planHash: "stable-hash",
    caskId: "ownership-cask",
    state: "DRAFT",
    permanent: false,
    cleanupOnly: true,
    expiresAtMs: Date.now() + 60000,
    items: [
      {
        id: "item",
        resourceId: "reference",
        name: "keep.txt",
        path: "D:/keep.txt",
        action: "DETACH",
        reasonCode: "NO_DELETE_AUTHORITY",
        state: "PENDING",
        recoveryPath: null,
      },
    ],
  });
  await screen.findByRole("heading", { name: "Cleanup Review" });
  expect(screen.getByText(/Remove from this Cask; keep the data/)).toBeTruthy();
  expect(screen.getByText("Review only")).toBeTruthy();
  await fireEvent.click(screen.getByRole("button", { name: "Review Cleanup" }));
  expect(
    screen.getByRole("status", { name: "Cleanup status: Reviewed" }),
  ).toBeTruthy();
  expect(
    (screen.getByRole("button", { name: "Reviewed" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  expect(
    screen
      .getByRole("button", { name: "Approve Cleanup" })
      .getAttribute("aria-describedby"),
  ).toBe("cleanup-limit");
  expect(
    calls.invoke.mock.calls.some(
      ([command]) => command === "execute_removal_plan",
    ),
  ).toBe(false);
  const advanced = screen.getByText("Advanced details").closest("details")!;
  expect(advanced.open).toBe(false);
  expect(screen.getByText(/Plan ID: stable-plan/).closest("details")).toBe(
    advanced,
  );
  expect(
    screen
      .getAllByText("D:/keep.txt")
      .some((node) => node.closest("details") === advanced),
  ).toBe(true);
  expect(
    (
      screen.getByRole("button", {
        name: "Approve Cleanup",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  expect(
    (
      screen.getByRole("button", {
        name: "Execute Cleanup",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
});

test("settings creates a metadata-only backup through the shared command", async () => {
  calls.invoke.mockImplementation(async (command: string) => {
    if (command === "list_casks" || command === "list_operations") return [];
    if (command === "recovery_status")
      return { version: 1, operations: [], pendingManifestCasks: [] };
    if (command === "backup_database")
      return { version: 1, path: "C:/backup.db", sizeBytes: 4096 };
    throw new Error(`Unexpected command: ${command}`);
  });
  render(ToastArea);
  render(Workspace, { mode: "settings" });
  await fireEvent.click(
    screen.getByRole("button", { name: "Back up database metadata" }),
  );
  await waitFor(() =>
    expect(calls.invoke).toHaveBeenCalledWith("backup_database", {
      request: { version: 1 },
    }),
  );
  expect((await screen.findByRole("status")).textContent).toContain(
    "Managed Cask files are not included",
  );
});

test("a dropped file waits for an explicit intake mode and Add action", async () => {
  const cask = {
    version: 1,
    id: "cask-2",
    name: "Work",
    state: "ACTIVE",
    managedStorageRoot: "C:/Casklets/cask-2",
    createdAtMs: 1,
    updatedAtMs: 1,
    revision: 0,
  };
  const resource = {
    version: 1,
    id: "resource-1",
    caskId: cask.id,
    name: "notes.txt",
    path: "C:/notes.txt",
    kind: "FILE",
    management: "REFERENCED",
    containment: "EXTERNAL",
    availability: "PRESENT",
    categoryKey: "other",
    sizeBytes: 5,
    sizeMeasurement: "EXACT",
    unknownEntries: 0,
  };
  calls.invoke.mockImplementation(async (command: string) => {
    if (command === "recovery_status")
      return { version: 1, operations: [], pendingManifestCasks: [] };
    if (command === "list_operations") return [];
    if (command === "list_casks") return [cask];
    if (command === "get_cask") return cask;
    if (command === "list_resources") return [];
    if (command === "list_categories")
      return [{ version: 1, key: "other", name: "Other", isBuiltin: true }];
    if (command === "storage_summary")
      return {
        managedLogicalBytes: 0,
        referencedLogicalBytes: 0,
        physicalBytes: null,
        reclaimableMinBytes: 0,
        reclaimableMaxBytes: 0,
        unknownEntries: 0,
      };
    if (command === "add_resource") return resource;
    throw new Error(`Unexpected command: ${command}`);
  });
  render(ToastArea);
  render(Workspace, { caskId: "cask-2" });
  await screen.findByRole("heading", { name: "Work" });
  await waitFor(() => expect(calls.drop).not.toBeNull());
  calls.drop?.({ payload: { type: "drop", paths: ["C:/notes.txt"] } });
  const add = await screen.findByRole("button", { name: "Add resource" });
  expect((add as HTMLButtonElement).disabled).toBe(true);
  expect(
    calls.invoke.mock.calls.some(([command]) => command === "add_resource"),
  ).toBe(false);
  await fireEvent.click(screen.getByLabelText("Reference at current location"));
  await fireEvent.click(add);
  await waitFor(() =>
    expect(calls.invoke).toHaveBeenCalledWith("add_resource", {
      request: {
        version: 1,
        caskId: "cask-2",
        sourcePath: "C:/notes.txt",
        mode: "reference",
        categoryKey: "other",
      },
    }),
  );
});

test("creates a Cask and opens a native detail window without replacing the library", async () => {
  const cask = {
    version: 1,
    id: "cask-1",
    name: "Research",
    description: "",
    state: "ACTIVE",
    managedStorageRoot: "C:/Casklets/cask-1",
    createdAtMs: 1,
    updatedAtMs: 1,
    revision: 0,
  };
  let items: (typeof cask)[] = [];
  calls.invoke.mockImplementation(async (command: string) => {
    if (command === "cask_library")
      return {
        version: 1,
        casks: items.map((cask) => ({
          cask,
          itemCount: 0,
          fileCount: 0,
          applicationCount: 0,
          managedBytes: 0,
          referencedBytes: 0,
          unknownEntries: 0,
        })),
        totalItems: 0,
        managedBytes: 0,
        unknownEntries: 0,
        storageRoot: "C:/Casklets",
      };
    if (command === "create_cask") {
      items = [cask];
      return cask;
    }
    if (command === "open_cask_window" || command === "window_ready") return;
    throw new Error(`Unexpected command: ${command}`);
  });
  render(App);
  await screen.findByText("No Casks yet");
  await fireEvent.click(screen.getByRole("button", { name: "New Cask" }));
  await fireEvent.input(screen.getByLabelText("Cask name"), {
    target: { value: "Research" },
  });
  await fireEvent.click(screen.getByRole("button", { name: "Create Cask" }));
  await waitFor(() =>
    expect(calls.invoke).toHaveBeenCalledWith("open_cask_window", {
      request: { version: 1, id: "cask-1" },
    }),
  );
  expect(screen.getByRole("heading", { name: "Casks" })).toBeTruthy();
  const row = await screen.findByRole("button", { name: "Open Research" });
  await fireEvent.click(row);
  expect(
    screen.queryByRole("navigation", { name: "Cask sections" }),
  ).toBeNull();
  await fireEvent.input(screen.getByLabelText("Search casks"), {
    target: { value: "Missing" },
  });
  expect(await screen.findByText("No matching Casks")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Open Research" })).toBeNull();
});

test("shows interrupted work and requires an explicit recovery action", async () => {
  const operation = {
    version: 1,
    id: "op-1",
    caskId: "cask-1",
    state: "RECOVERY_REQUIRED",
    step: "RECONCILE",
    progressCompleted: 1,
    progressTotal: 5,
    cancellationRequested: false,
    errorCode: "INTERRUPTED",
    updatedAtMs: 1,
  };
  let pending = true;
  calls.invoke.mockImplementation(async (command: string) => {
    if (command === "list_casks") return [];
    if (command === "list_operations") return [operation];
    if (command === "recovery_status")
      return {
        version: 1,
        operations: pending ? [operation] : [],
        pendingManifestCasks: [],
      };
    if (command === "inspect_recovery")
      return {
        version: 1,
        operation,
        mode: "MOVE",
        sourcePath: "C:/source.txt",
        sourceStatus: "MISSING",
        stagingPath: "C:/stage.txt",
        stagingStatus: "PRESENT_FILE",
        finalPath: "C:/final.txt",
        finalStatus: "MISSING",
      };
    if (command === "preserve_operation") {
      pending = false;
      return { ...operation, state: "CLOSED_PRESERVED" };
    }
    throw new Error(`Unexpected command: ${command}`);
  });
  render(ToastArea);
  render(Workspace, { mode: "activity" });
  await screen.findByText("Recovery needed");
  await fireEvent.click(screen.getByRole("button", { name: "Inspect paths" }));
  expect(await screen.findByText("C:/stage.txt (PRESENT_FILE)")).toBeTruthy();
  await fireEvent.click(
    screen.getByRole("button", { name: "Keep files and close" }),
  );
  await waitFor(() => expect(screen.queryByText("Recovery needed")).toBeNull());
});

test.each([
  [false, false],
  [true, false],
  [false, true],
])(
  "cleanup execution requires review, approval, and explicit confirmation (permanent: %s, interrupted: %s)",
  async (permanent, interrupted) => {
    const cask = {
      version: 1,
      id: "cask-1",
      name: "Work",
      state: "ACTIVE",
      managedStorageRoot: "C:/Casklets/cask-1",
      createdAtMs: 1,
      updatedAtMs: 1,
      revision: 0,
    };
    const item = {
      id: "item-1",
      resourceId: "resource-1",
      name: "notes.txt",
      path: "C:/notes.txt",
      action: permanent ? "DELETE_PERMANENTLY" : "TRASH",
      reasonCode: "ELIGIBLE_MANAGED",
      expectedSizeBytes: 5,
      state: "PENDING",
      resultCode: null,
      recoveryPath: null,
    };
    const draft = {
      version: 1,
      id: "plan-1",
      caskId: cask.id,
      scope: "CASK",
      categoryKey: null,
      permanent,
      state: "DRAFT",
      planHash: "hash-1",
      runId: null,
      createdAtMs: 1,
      expiresAtMs: Date.now() + 100000,
      items: [item],
    };
    calls.invoke.mockImplementation(async (command: string) => {
      if (command === "removal_capability") return true;
      if (command === "list_casks") return [cask];
      if (command === "cask_removal_status")
        return {
          version: 1,
          canRemove: false,
          resourceCount: 1,
          applicationCount: 0,
          unfinishedOperations: 0,
        };
      if (command === "get_cask") return cask;
      if (command === "recovery_status")
        return { version: 1, operations: [], pendingManifestCasks: [] };
      if (command === "list_resources")
        return [
          {
            id: "resource-1",
            name: "notes.txt",
            caskId: cask.id,
            kind: "FILE",
            path: "C:/notes.txt",
            management: "MANAGED",
            containment: "INTERNAL",
            availability: "PRESENT",
            categoryKey: "other",
            sizeBytes: 5,
            sizeMeasurement: "EXACT",
            unknownEntries: 0,
            ownership: "OWNED",
            provenance: "ADDED_BY_CASKLET",
          },
        ];
      if (command === "list_operations" || command === "list_categories")
        return [];
      if (command === "storage_summary")
        return {
          managedLogicalBytes: 0,
          referencedLogicalBytes: 0,
          physicalBytes: null,
          reclaimableMinBytes: 0,
          reclaimableMaxBytes: 0,
          unknownEntries: 0,
        };
      if (command === "create_removal_plan") return draft;
      if (command === "approve_removal_plan")
        return { ...draft, state: "APPROVED" };
      if (command === "get_removal_plan")
        return {
          ...draft,
          state: "PARTIAL",
          runId: "run-1",
          items: [{ ...item, state: "UNKNOWN", resultCode: "INTERRUPTED" }],
        };
      if (command === "execute_removal_plan" && interrupted)
        return Promise.reject({
          version: 1,
          code: "RECOVERY_REQUIRED",
          message: "Cleanup needs recovery",
          correlationId: "test",
        });
      if (command === "execute_removal_plan")
        return {
          ...draft,
          state: "COMPLETE",
          runId: "run-1",
          items: [{ ...item, state: "SUCCEEDED", resultCode: "TRASHED" }],
        };
      throw new Error(`Unexpected command: ${command}`);
    });
    render(ToastArea);
    render(Workspace, { caskId: "cask-1" });
    await screen.findByRole("heading", { name: "Work" });
    await fireEvent.click(
      await screen.findByRole("button", { name: "Cleanup" }),
    );
    if (permanent)
      await fireEvent.click(
        await screen.findByRole("radio", {
          name: "Permanently delete eligible files",
        }),
      );
    await fireEvent.click(
      await screen.findByRole("button", { name: "Create cleanup plan" }),
    );
    const approve = await screen.findByRole("button", {
      name: "Approve Cleanup",
    });
    expect((approve as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.click(
      screen.getByRole("button", { name: "Review Cleanup" }),
    );
    await fireEvent.click(approve);
    const execute = await screen.findByRole("button", {
      name: "Execute Cleanup",
    });
    expect((execute as HTMLButtonElement).disabled).toBe(true);
    expect(
      calls.invoke.mock.calls.some(
        ([command]) => command === "execute_removal_plan",
      ),
    ).toBe(false);
    expect(screen.queryByLabelText("Type the plan ID to execute")).toBeNull();
    await fireEvent.click(
      screen.getByLabelText("I confirm the cleanup actions shown above"),
    );
    if (permanent) {
      expect((execute as HTMLButtonElement).disabled).toBe(true);
      await fireEvent.click(
        screen.getByLabelText(
          "I understand permanent deletion cannot be undone",
        ),
      );
    }
    await fireEvent.click(execute);
    await waitFor(() =>
      expect(calls.invoke).toHaveBeenCalledWith("execute_removal_plan", {
        request: {
          version: 1,
          id: "plan-1",
          planHash: "hash-1",
          confirmPermanent: permanent,
        },
      }),
    );
    if (interrupted) {
      await screen.findByRole("status", {
        name: "Cleanup status: Needs attention",
      });
      expect(
        screen.queryByRole("button", { name: "Execute Cleanup" }),
      ).toBeNull();
      expect(
        (
          screen.getByRole("button", {
            name: "Create cleanup plan",
          }) as HTMLButtonElement
        ).disabled,
      ).toBe(true);
    }
  },
);

test("global notifications replace keyed updates and expire without occupying content", async () => {
  vi.useFakeTimers();
  render(ToastArea);
  notify("Resource added", "success", "operation");
  await vi.advanceTimersByTimeAsync(0);
  expect(screen.getByRole("status").textContent).toContain("Resource added");
  notify("Changes saved", "success", "operation");
  await vi.advanceTimersByTimeAsync(0);
  expect(screen.getAllByRole("status")).toHaveLength(1);
  expect(screen.getByRole("status").textContent).toContain("Changes saved");
  await vi.advanceTimersByTimeAsync(4600);
  expect(screen.queryByRole("status")).toBeNull();
});

test("window readiness waits for layout before requesting native visibility", async () => {
  vi.useFakeTimers();
  calls.invoke.mockResolvedValue(undefined);
  const ready = revealWindow();
  expect(calls.invoke).not.toHaveBeenCalled();
  await vi.runAllTimersAsync();
  await ready;
  expect(calls.invoke).toHaveBeenCalledWith("window_ready");
});

test("sort direction toggles and Cask menus dismiss and replace one another", async () => {
  calls.invoke.mockImplementation(async (command: string) => {
    if (command === "window_ready") return;
    if (command === "cask_library")
      return {
        version: 1,
        totalItems: 3,
        managedBytes: 30,
        unknownEntries: 0,
        storageRoot: "C:/Casklets",
        casks: ["Alpha", "Beta"].map((name, index) => ({
          cask: {
            version: 1,
            id: `cask-${index}`,
            name,
            description: "",
            state: "ACTIVE",
            managedStorageRoot: `C:/Casklets/${index}`,
            createdAtMs: 1,
            updatedAtMs: index + 1,
            revision: 0,
          },
          itemCount: index + 1,
          fileCount: index + 1,
          applicationCount: 0,
          managedBytes: (index + 1) * 10,
          referencedBytes: 0,
          unknownEntries: 0,
        })),
      };
    throw new Error(`Unexpected command: ${command}`);
  });
  render(App);
  await screen.findByRole("button", { name: "Open Alpha" });
  const items = screen.getByRole("button", { name: /Items/ });
  await fireEvent.click(items);
  expect(items.getAttribute("data-direction")).toBe("ascending");
  await fireEvent.click(items);
  expect(items.getAttribute("data-direction")).toBe("descending");
  expect(screen.queryByRole("button", { name: /Actions for/ })).toBeNull();
  expect(screen.queryByRole("menu")).toBeNull();
});

test("Cask accents are stable and varied by identity", () => {
  expect(caskColor("cask-1")).toBe(caskColor("cask-1"));
  expect(
    new Set(
      Array.from({ length: 12 }, (_, index) => caskColor(`cask-${index}`)),
    ).size,
  ).toBeGreaterThan(3);
});

test("application-only Casks are not removable and cleanup directs users to applications", async () => {
  ownershipWorkspace([], (command) =>
    command === "cask_removal_status"
      ? {
          version: 1,
          canRemove: false,
          resourceCount: 0,
          applicationCount: 1,
          unfinishedOperations: 0,
        }
      : undefined,
  );
  await screen.findByRole("heading", { name: "Ownership" });
  expect(screen.queryByRole("button", { name: "Remove Cask" })).toBeNull();
  await fireEvent.click(screen.getByRole("button", { name: "Cleanup" }));
  await screen.findByRole("button", { name: "Review applications" });
  expect(
    screen.queryByRole("button", { name: "Create cleanup plan" }),
  ).toBeNull();
  expect(
    calls.invoke.mock.calls.some(
      ([command]) => command === "create_removal_plan",
    ),
  ).toBe(false);
});

test("resource intake hides Copy and Move and Settings retains Archive without storage location", async () => {
  ownershipWorkspace();
  await screen.findByRole("heading", { name: "Ownership" });
  await waitFor(() => expect(calls.drop).not.toBeNull());
  calls.drop?.({ payload: { type: "drop", paths: ["D:/project.txt"] } });
  await screen.findByLabelText("Reference at current location");
  expect(screen.queryByRole("radio", { name: /^Copy/ })).toBeNull();
  expect(screen.queryByRole("radio", { name: /^Move/ })).toBeNull();
  expect(screen.queryByRole("button", { name: "Archive Cask" })).toBeNull();
  const nav = screen.getByRole("navigation", { name: "Cask sections" });
  await fireEvent.click(within(nav).getByRole("button", { name: /Settings/ }));
  expect(screen.getByLabelText("Archive this Cask")).toBeTruthy();
  expect(screen.queryByLabelText("Storage location")).toBeNull();
});

test("deletion method is visible and changing it invalidates the reviewed preview", async () => {
  ownershipWorkspace(
    [
      {
        id: "resource",
        name: "notes.txt",
        kind: "FILE",
        categoryKey: "other",
        sizeBytes: 5,
      },
    ],
    (command) =>
      command === "create_removal_plan"
        ? {
            version: 1,
            id: "permanent-plan",
            caskId: "ownership-cask",
            state: "DRAFT",
            planHash: "permanent-hash",
            permanent: true,
            cleanupOnly: true,
            expiresAtMs: Date.now() + 60000,
            items: [
              {
                id: "item",
                name: "notes.txt",
                path: "C:/notes.txt",
                action: "DELETE_PERMANENTLY",
                reasonCode: "ELIGIBLE_MANAGED",
                state: "PENDING",
              },
            ],
          }
        : undefined,
  );
  await screen.findByRole("heading", { name: "Ownership" });
  await fireEvent.click(screen.getByRole("button", { name: "Cleanup" }));
  const permanent = await screen.findByRole("radio", {
    name: "Permanently delete eligible files",
  });
  expect(permanent.closest("details")).toBeNull();
  await fireEvent.click(permanent);
  expect(screen.getByText(/Cannot be undone/)).toBeTruthy();
  await fireEvent.click(
    screen.getByRole("button", { name: "Create cleanup plan" }),
  );
  await screen.findByRole("heading", { name: "Cleanup Review" });
  expect(calls.invoke).toHaveBeenCalledWith("create_removal_plan", {
    request: {
      version: 1,
      caskId: "ownership-cask",
      resourceIds: [],
      categoryKey: null,
      permanent: true,
    },
  });
  await fireEvent.click(screen.getByRole("button", { name: "Review Cleanup" }));
  await fireEvent.click(
    screen.getByRole("radio", { name: "Move to Recycle Bin" }),
  );
  expect(screen.queryByRole("heading", { name: "Cleanup Review" })).toBeNull();
  expect(
    calls.invoke.mock.calls.some(([command]) =>
      ["approve_removal_plan", "execute_removal_plan"].includes(command),
    ),
  ).toBe(false);
});

test("interrupted cleanup is rediscovered and closure needs explicit preservation confirmation", async () => {
  const partial = {
    version: 1,
    id: "interrupted",
    caskId: "ownership-cask",
    state: "PARTIAL",
    scope: "CASK",
    permanent: false,
    cleanupOnly: true,
    planHash: "hash",
    runId: "run",
    createdAtMs: 1,
    expiresAtMs: Date.now() + 60000,
    items: [
      {
        id: "item",
        resourceId: "resource",
        name: "notes.txt",
        path: "D:/notes.txt",
        action: "TRASH",
        state: "UNKNOWN",
        reasonCode: "ELIGIBLE_MANAGED",
        recoveryPath: "D:/.casklet-removal-run-item",
      },
    ],
  };
  let closed = false;
  ownershipWorkspace(
    [
      {
        id: "resource",
        name: "notes.txt",
        kind: "FILE",
        categoryKey: "other",
        sizeBytes: 5,
      },
    ],
    (command) => {
      if (command === "removal_capability") return true;
      if (command === "list_removal_plans")
        return [{ ...partial, state: closed ? "COMPLETE" : "PARTIAL" }];
      if (command === "close_uncertain_removal") {
        closed = true;
        return { ...partial, state: "COMPLETE" };
      }
      return undefined;
    },
  );
  await screen.findByRole("heading", { name: "Ownership" });
  await fireEvent.click(screen.getByRole("button", { name: "Cleanup" }));
  await screen.findByRole("status", {
    name: "Cleanup status: Needs attention",
  });
  const close = screen.getByRole("button", {
    name: "Keep unresolved items and close cleanup",
  });
  expect((close as HTMLButtonElement).disabled).toBe(true);
  expect(
    (
      screen.getByRole("button", {
        name: "Create cleanup plan",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  const advanced = screen.getByText("Advanced details").closest("details")!;
  await fireEvent.click(within(advanced).getByText("Advanced details"));
  expect(
    within(advanced).getByText("D:/.casklet-removal-run-item"),
  ).toBeTruthy();
  await fireEvent.click(
    screen.getByLabelText(
      "I inspected the recovery locations and will keep unresolved items",
    ),
  );
  await fireEvent.click(close);
  await waitFor(() =>
    expect(calls.invoke).toHaveBeenCalledWith("close_uncertain_removal", {
      request: { version: 1, runId: "run" },
    }),
  );
  await screen.findByText(
    "Cleanup closed. Unresolved items still need inspection.",
  );
  expect(
    calls.invoke.mock.calls.some(
      ([command]) => command === "execute_removal_plan",
    ),
  ).toBe(false);
});

test("an expired saved plan cannot be reviewed, approved, or executed", async () => {
  const plan = {
    version: 1,
    id: "expired",
    caskId: "ownership-cask",
    state: "APPROVED",
    scope: "CASK",
    planHash: "hash",
    permanent: false,
    cleanupOnly: true,
    createdAtMs: 1,
    expiresAtMs: 2,
    items: [
      {
        id: "item",
        resourceId: "resource",
        name: "notes.txt",
        path: "D:/notes.txt",
        action: "TRASH",
        reasonCode: "ELIGIBLE_MANAGED",
        state: "PENDING",
      },
    ],
  };
  ownershipWorkspace(
    [
      {
        id: "resource",
        name: "notes.txt",
        kind: "FILE",
        categoryKey: "other",
        sizeBytes: 5,
      },
    ],
    (command) => {
      if (command === "list_removal_plans") return [plan];
      if (command === "removal_capability") return true;
      return undefined;
    },
  );
  await screen.findByRole("heading", { name: "Ownership" });
  await fireEvent.click(screen.getByRole("button", { name: "Cleanup" }));
  await fireEvent.click(await screen.findByText("Recent cleanups"));
  await fireEvent.click(
    screen.getByRole("button", { name: /Approved.*1 items/ }),
  );
  await screen.findByRole("status", { name: "Cleanup status: Expired" });
  expect(
    screen
      .getByRole("status", { name: "Cleanup status: Expired" })
      .closest("details")
      ?.querySelector("summary")?.textContent,
  ).toBe("Recent cleanups");
  for (const name of ["Review Cleanup", "Approve Cleanup", "Execute Cleanup"])
    expect(
      (screen.getByRole("button", { name }) as HTMLButtonElement).disabled,
    ).toBe(true);
  expect(
    screen.getByText("Plan expired. Create a new cleanup plan."),
  ).toBeTruthy();
  expect(
    screen.queryByLabelText("I confirm the cleanup actions shown above"),
  ).toBeNull();
});

test("installer artifacts stay out of Files and Cask-installed EXE provenance stays distinct", async () => {
  const artifact = {
    version: 1,
    id: "installer-resource",
    kind: "FILE",
    name: "setup.exe",
    path: "C:/Casklets/test/setup.exe",
    management: "MANAGED",
    containment: "INTERNAL",
    availability: "PRESENT",
    relationship: "OWNED",
    removalPolicy: "DELETE_WHEN_UNREFERENCED",
    categoryKey: "applications",
    sizeBytes: 1,
    sizeQuality: "EXACT",
    revision: 0,
  };
  const exe = {
    ...appFixture("Installed EXE", "exe-app"),
    provenance: "INSTALLED_BY_CASKLET",
    removalPolicy: "CASKLET_OWNED",
  };
  ownershipWorkspace([artifact], (command) => {
    if (command === "list_installer_artifacts")
      return [{ id: "artifact", resourceId: artifact.id }];
    if (command === "list_associated_applications") return [exe];
    if (command === "create_application_removal")
      return {
        version: 1,
        id: "exe-plan",
        caskId: "ownership-cask",
        state: "DRAFT",
        planHash: "hash",
        expiresAtMs: Date.now() + 600000,
        items: [
          {
            installationId: "exe-app",
            name: "Installed EXE",
            productCode: "registry",
            evidenceSha256: "evidence",
            uninstallerSha256: "exe-hash",
            result: null,
            action: "UNINSTALL",
            state: "PENDING",
          },
        ],
      };
    return undefined;
  });
  await screen.findByRole("heading", { name: "Ownership" });
  await waitFor(() =>
    expect(calls.invoke).toHaveBeenCalledWith(
      "list_installer_artifacts",
      expect.anything(),
    ),
  );
  await fireEvent.click(
    screen.getByRole("button", { name: "Files & Folders" }),
  );
  expect(screen.queryByText("setup.exe")).toBeNull();
  await fireEvent.click(screen.getByRole("button", { name: "Applications" }));
  await screen.findByText("Installed EXE");
  expect(screen.getByText("Casklet-owned")).toBeTruthy();
  expect(
    screen.getByRole("button", {
      name: "Remove Installed EXE",
    }),
  ).toBeTruthy();
  expect(applicationRows([], [exe], [])[0].management).toBe("managed");
  expect(exe.removalPolicy).toBe("CASKLET_OWNED");
  await fireEvent.click(
    screen.getByRole("button", { name: "Remove Installed EXE" }),
  );
  await fireEvent.click(
    await screen.findByRole("button", {
      name: "Preview selected application removal",
    }),
  );
  expect(calls.invoke).toHaveBeenCalledWith("create_application_removal", {
    request: {
      version: 1,
      caskId: "ownership-cask",
      installationId: "exe-app",
    },
  });
  expect(
    calls.invoke.mock.calls.some(
      ([command]) => command === "disassociate_installed_application",
    ),
  ).toBe(false);
});

test("Cask storage includes current owned and reference applications", async () => {
  const cask = {
    version: 1,
    id: "storage-cask",
    name: "Storage test",
    state: "ACTIVE",
    managedStorageRoot: "C:/Casklets/storage",
    createdAtMs: 1,
    updatedAtMs: 1,
    revision: 0,
  };
  calls.invoke.mockImplementation(async (command: string) => {
    if (command === "get_cask") return cask;
    if (command === "list_casks") return [cask];
    if (command === "recovery_status")
      return { version: 1, operations: [], pendingManifestCasks: [] };
    if (command === "cask_removal_status")
      return {
        canRemove: false,
        resourceCount: 0,
        applicationCount: 2,
        unfinishedOperations: 0,
      };
    if (command === "storage_summary")
      return {
        version: 1,
        managedLogicalBytes: 1024,
        referencedLogicalBytes: 1024,
        ownedApplicationBytes: 2048,
        referenceApplicationBytes: 4096,
        estimatedApplications: 1,
        unknownEntries: 0,
        physicalBytes: null,
        reclaimableMinBytes: 0,
        reclaimableMaxBytes: 1024,
      };
    if (command === "removal_capability") return false;
    return [];
  });
  render(Workspace, { caskId: cask.id });
  await screen.findByRole("heading", { name: cask.name });
  await screen.findByText("8 KB");
  expect(screen.getByText("2 KB")).toBeTruthy();
  expect(screen.getByText("5 KB")).toBeTruthy();
  expect(screen.getByText(/1 use Windows size estimates/)).toBeTruthy();
});
