import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/svelte";
import { afterEach, expect, test, vi } from "vitest";
import App from "./App.svelte";

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
  calls.invoke.mockReset();
  calls.drop = null;
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
  render(App);
  await fireEvent.click(await screen.findByRole("button", { name: "Work" }));
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

test("creates and inspects a Cask through typed Tauri commands", async () => {
  const cask = {
    version: 1,
    id: "cask-1",
    name: "Research",
    state: "ACTIVE",
    managedStorageRoot: "C:/Casklets/cask-1",
    createdAtMs: 1,
    updatedAtMs: 1,
    revision: 0,
  };
  let items: (typeof cask)[] = [];
  calls.invoke.mockImplementation(async (command: string) => {
    if (command === "recovery_status")
      return { version: 1, operations: [], pendingManifestCasks: [] };
    if (command === "list_operations") return [];
    if (command === "list_casks") return items;
    if (command === "create_cask") {
      items = [cask];
      return cask;
    }
    if (command === "get_cask") return cask;
    if (command === "list_resources" || command === "list_categories")
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
    throw new Error(`Unexpected command: ${command}`);
  });
  render(App);
  await screen.findByText("No Casks yet.");
  await fireEvent.input(screen.getByLabelText("New Cask name"), {
    target: { value: "Research" },
  });
  await fireEvent.click(screen.getByRole("button", { name: "Create Cask" }));
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Research" })).toBeTruthy(),
  );
  await fireEvent.click(screen.getByRole("button", { name: "Research" }));
  await waitFor(() =>
    expect(calls.invoke).toHaveBeenCalledWith("get_cask", {
      request: { version: 1, id: "cask-1" },
    }),
  );
  expect(screen.getByText("C:/Casklets/cask-1")).toBeTruthy();
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
  render(App);
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
  render(App);
  await fireEvent.click(await screen.findByRole("button", { name: "Work" }));
  await fireEvent.click(
    screen.getByRole("button", { name: "Create cleanup preview" }),
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
