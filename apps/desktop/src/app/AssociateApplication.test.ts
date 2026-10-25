import { afterEach, expect, test, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/svelte";
import AssociateApplication from "./AssociateApplication.svelte";
import type {
  CaskView,
  DetectedApplicationView,
} from "../contracts/generated/casklet";
const native = vi.hoisted(() => ({
  invoke: vi.fn(),
  emit: vi.fn(async () => {}),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: native.invoke }));
vi.mock("@tauri-apps/api/event", () => ({
  emit: native.emit,
  listen: vi.fn(async () => () => {}),
}));
HTMLDialogElement.prototype.showModal = function () {
  this.setAttribute("open", "");
};
const application: DetectedApplicationView = {
  version: 1,
  applicationId: "app",
  installationId: "installation",
  name: "Example app",
  publisher: null,
  displayVersion: "1",
  scope: "USER",
  sourceKey: "registry-key",
  installLocation: null,
  state: "PRESENT",
  ambiguousName: false,
  associatedCaskId: null,
  provenance: "DETECTED_WINDOWS_REGISTRY",
  removalPolicy: "DETACH_ONLY",
};
const cask = (id: string, state = "ACTIVE"): CaskView => ({
  version: 1,
  id,
  name: `Cask ${id}`,
  description: "",
  state,
  managedStorageRoot: `C:/${id}`,
  createdAtMs: 1,
  updatedAtMs: 1,
  revision: 0,
});
afterEach(() => {
  cleanup();
  localStorage.clear();
  native.invoke.mockReset();
  native.emit.mockClear();
});
function setup(casks: CaskView[], linked: string[] = []) {
  native.invoke.mockImplementation(async (command, args) => {
    if (command === "list_casks") {
      expect(args).toEqual({ request: { version: 1 } });
      return casks;
    }
    if (command === "list_associated_applications")
      return linked.includes(args.request.caskId) ? [application] : [];
    return application;
  });
  const callbacks = {
    onClose: vi.fn(),
    onAssociated: vi.fn(),
    onCreate: vi.fn(),
  };
  render(AssociateApplication, { application, ...callbacks });
  return callbacks;
}
test("chooses an active destination, prevents duplicates and only calls the existing reference command", async () => {
  const callbacks = setup(
    [cask("linked"), cask("target"), cask("archived", "ARCHIVED")],
    ["linked"],
  );
  const target = await screen.findByRole("radio", { name: "Cask target" });
  expect(
    screen.getByRole("radio", { name: /Cask linked/ }).hasAttribute("disabled"),
  ).toBe(true);
  expect(screen.queryByText("Cask archived")).toBeNull();
  const button = screen.getByRole("button", { name: "Associate" });
  expect(button.hasAttribute("disabled")).toBe(true);
  await fireEvent.click(target);
  let finish: (value: DetectedApplicationView) => void = () => {};
  native.invoke.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  await fireEvent.click(button);
  await fireEvent.click(button);
  expect(
    native.invoke.mock.calls.filter(
      ([command]) => command === "associate_installed_application",
    ),
  ).toHaveLength(1);
  expect(native.invoke).toHaveBeenLastCalledWith(
    "associate_installed_application",
    {
      request: { version: 1, caskId: "target", installationId: "installation" },
    },
  );
  finish(application);
  await waitFor(() =>
    expect(callbacks.onAssociated).toHaveBeenCalledWith(cask("target")),
  );
  expect(
    native.invoke.mock.calls.some(([command]) =>
      /install_application|set.*ownership|disassociate/.test(command),
    ),
  ).toBe(false);
});
test("failed association retains destination and allows retry", async () => {
  const callbacks = setup([cask("target")]);
  await fireEvent.click(
    await screen.findByRole("radio", { name: "Cask target" }),
  );
  native.invoke.mockRejectedValueOnce({
    message: "Application cleanup is active",
  });
  await fireEvent.click(screen.getByRole("button", { name: "Associate" }));
  expect((await screen.findByRole("alert")).textContent).toContain(
    "Application cleanup is active",
  );
  expect((screen.getByRole("radio") as HTMLInputElement).checked).toBe(true);
  expect(callbacks.onAssociated).not.toHaveBeenCalled();
  await fireEvent.click(screen.getByRole("button", { name: "Associate" }));
  await waitFor(() => expect(callbacks.onAssociated).toHaveBeenCalledOnce());
});
test("empty and fully linked destinations provide creation as the next step", async () => {
  const callbacks = setup([]);
  await screen.findByText("No active Casks");
  await fireEvent.click(screen.getByRole("button", { name: "Create a Cask" }));
  expect(callbacks.onCreate).toHaveBeenCalledOnce();
  cleanup();
  setup([cask("linked")], ["linked"]);
  await screen.findByText(
    "This application is already associated with every active Cask.",
  );
  expect(
    screen.getByRole("button", { name: "Associate" }).hasAttribute("disabled"),
  ).toBe(true);
});
test("unverified destinations stay disabled until rechecked", async () => {
  setup([cask("target")]);
  native.invoke.mockRejectedValueOnce({ message: "Unavailable" });
  await screen.findByRole("alert");
  expect(screen.getByRole("radio").hasAttribute("disabled")).toBe(true);
  await fireEvent.click(screen.getByRole("button", { name: "Retry Casks" }));
  await waitFor(() =>
    expect(screen.getByRole("radio").hasAttribute("disabled")).toBe(false),
  );
});

test("filtering destinations clears a selection before it becomes hidden", async () => {
  setup([cask("one"), cask("two")]);
  await fireEvent.click(await screen.findByRole("radio", { name: "Cask one" }));
  const associate = screen.getByRole("button", { name: "Associate" });
  expect(associate.hasAttribute("disabled")).toBe(false);
  await fireEvent.input(
    screen.getByRole("searchbox", { name: "Search Casks" }),
    { target: { value: "two" } },
  );
  expect(screen.queryByRole("radio", { name: "Cask one" })).toBeNull();
  expect(associate.hasAttribute("disabled")).toBe(true);
});
