import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
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

test("cleanup execution requires the exact reviewed plan ID", async () => {
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
    action: "TRASH",
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
    permanent: false,
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
    if (command === "get_cask") return cask;
    if (command === "recovery_status")
      return { version: 1, operations: [], pendingManifestCasks: [] };
    if (
      command === "list_operations" ||
      command === "list_resources" ||
      command === "list_categories"
    )
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
    screen.getByRole("button", { name: "Create Cleanup Plan" }),
  );
  await fireEvent.click(
    await screen.findByRole("button", { name: "Approve this plan" }),
  );
  const execute = await screen.findByRole("button", {
    name: "Execute cleanup",
  });
  expect((execute as HTMLButtonElement).disabled).toBe(true);
  expect(
    calls.invoke.mock.calls.some(
      ([command]) => command === "execute_removal_plan",
    ),
  ).toBe(false);
  await fireEvent.input(screen.getByLabelText("Type the plan ID to execute"), {
    target: { value: "plan-1" },
  });
  await fireEvent.click(execute);
  await waitFor(() =>
    expect(calls.invoke).toHaveBeenCalledWith("execute_removal_plan", {
      request: {
        version: 1,
        id: "plan-1",
        planHash: "hash-1",
        confirmPermanent: false,
      },
    }),
  );
});

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
