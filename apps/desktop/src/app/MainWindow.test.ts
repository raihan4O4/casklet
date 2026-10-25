import { afterEach, expect, test, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/svelte";
import MainWindow from "./MainWindow.svelte";
import { clearNotices } from "./notifications";

const native = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
  emit: vi.fn(async () => {}),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn(), open: vi.fn() }));
HTMLDialogElement.prototype.showModal = function () {
  this.setAttribute("open", "");
};

afterEach(() => {
  cleanup();
  clearNotices();
  vi.restoreAllMocks();
  native.invoke.mockReset();
});

const library = {
  version: 1,
  storageRoot: "C:/Casks",
  totalItems: 0,
  managedBytes: 0,
  unknownEntries: 0,
  casks: [
    {
      cask: {
        version: 1,
        id: "slow-cask",
        name: "Slow library",
        description: "",
        state: "ACTIVE",
        createdAtMs: 1,
        updatedAtMs: 1,
        revision: 0,
      },
      itemCount: 0,
      managedBytes: 0,
      referencedBytes: 0,
      unknownEntries: 0,
    },
  ],
};

test("a scan that outlasts repeated polling intervals still loads instead of discarding every result", async () => {
  let finish: (value: typeof library) => void = () => {};
  const pending = new Promise<typeof library>((resolve) => {
    finish = resolve;
  });
  native.invoke.mockImplementation(async (command: string) =>
    command === "cask_library"
      ? pending
      : { operations: [], pendingManifestCasks: [] },
  );
  const intervals = vi.spyOn(window, "setInterval");
  const ready = vi.fn();
  render(MainWindow, { onReady: ready });
  await waitFor(() =>
    expect(native.invoke).toHaveBeenCalledWith(
      "cask_library",
      expect.anything(),
    ),
  );
  const poll = intervals.mock.calls.find(([, delay]) => delay === 5000)![0];
  expect(ready).toHaveBeenCalledOnce();
  expect(screen.getByRole("status").textContent).toContain("Loading Casks");
  if (typeof poll !== "function") throw Error("Missing library poll");
  // Trigger three poll ticks while the original native request remains pending.
  poll();
  poll();
  poll();
  expect(
    native.invoke.mock.calls.filter(([command]) => command === "cask_library"),
  ).toHaveLength(1);
  finish(library);
  await screen.findByRole("button", { name: "Open Slow library" });
  expect(screen.queryByText("Couldn’t load your Casks")).toBeNull();
  poll();
  await waitFor(() =>
    expect(
      native.invoke.mock.calls.filter(
        ([command]) => command === "cask_library",
      ),
    ).toHaveLength(2),
  );
});

test("load errors remain inline and Retry shows progress then accepts the successful response", async () => {
  let finish: (value: typeof library) => void = () => {};
  native.invoke.mockRejectedValueOnce({
    code: "INVENTORY",
    message: "Cannot read installed application inventory",
  });
  render(MainWindow);
  const alert = await screen.findByRole("alert");
  expect(alert.textContent).toContain(
    "Cannot read installed application inventory",
  );
  clearNotices();
  expect(screen.getByRole("alert")).toBe(alert);
  native.invoke.mockImplementation(async (command: string) =>
    command === "cask_library"
      ? new Promise((resolve) => {
          finish = resolve;
        })
      : { operations: [], pendingManifestCasks: [] },
  );
  await fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  await screen.findByRole("status");
  expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
  finish(library);
  await screen.findByRole("button", { name: "Open Slow library" });
  expect(screen.queryByRole("alert")).toBeNull();
});

test("the installations action associates a reference and updates its row", async () => {
  const installation = {
    installationId: "installation",
    name: "Editor",
    state: "PRESENT",
    scope: "USER",
    sourceKey: "registry-key",
    publisher: "Publisher",
    displayVersion: "1",
  };
  native.invoke.mockImplementation(async (command) => {
    if (command === "cask_library") return library;
    if (command === "list_casks") return [library.casks[0].cask];
    if (command === "search_installed_applications") return [installation];
    if (command === "list_associated_applications") return [];
    if (command === "associate_installed_application") return installation;
    return { operations: [], pendingManifestCasks: [] };
  });
  render(MainWindow);
  await screen.findByRole("button", { name: "Open Slow library" });
  expect(screen.queryByText("Your library")).toBeNull();
  expect(screen.getByLabelText("Library summary").textContent).toMatch(
    /Casks\s*1/,
  );
  await fireEvent.click(screen.getByRole("button", { name: "Installations" }));
  const origin = await screen.findByRole("button", {
    name: "Associate Editor",
  });
  expect(screen.queryByText("Present")).toBeNull();
  await fireEvent.click(origin);
  await fireEvent.click(
    await screen.findByRole("radio", { name: "Slow library" }),
  );
  await fireEvent.click(screen.getByRole("button", { name: "Associate" }));
  await screen.findByText("Associated with Slow library");
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(document.activeElement).toBe(origin);
  expect(native.invoke).toHaveBeenCalledWith(
    "associate_installed_application",
    {
      request: {
        version: 1,
        caskId: "slow-cask",
        installationId: "installation",
      },
    },
  );
});
