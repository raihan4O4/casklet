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
import ToastArea from "./ToastArea.svelte";
import { clearNotices, notify } from "./notifications";
import { revealWindow } from "./windowReady";
import { caskColor } from "./format";
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
  drop: null as
    | null
    | ((event: { payload: { type: string; paths: string[] } }) => void),
}));
vi.mock("@tauri-apps/api/core", () => calls);
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
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
  expect(
    calls.invoke.mock.calls.some(([command]) => command === "remove_cask"),
  ).toBe(false);
  await fireEvent.click(
    screen.getByRole("button", { name: "Confirm Remove Cask" }),
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
  const triggers = screen.getAllByRole("button", { name: /Actions for/ });
  await fireEvent.click(triggers[0]);
  expect(screen.getAllByRole("menu")).toHaveLength(1);
  await fireEvent.click(triggers[1]);
  await waitFor(() => expect(screen.getAllByRole("menu")).toHaveLength(1));
  expect(triggers[0].getAttribute("aria-expanded")).toBe("false");
  await fireEvent.pointerDown(document.body);
  await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  await fireEvent.click(triggers[0]);
  await fireEvent.keyDown(document, { key: "Escape" });
  await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  expect(document.activeElement).toBe(triggers[0]);
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
