import type {
  DetectedApplicationView,
  PortableInstallationView,
  WingetInstallationView,
} from "../contracts/generated/casklet";

type AppPresentation = {
  id: string;
  name: string;
  version: string | null;
  publisher: string | null;
  management: "managed" | "associated" | "unverified";
  provider: string;
};
export type ApplicationRow = AppPresentation &
  (
    | { kind: "portable"; item: PortableInstallationView }
    | { kind: "windows"; item: DetectedApplicationView }
    | { kind: "winget"; item: WingetInstallationView }
  );
export type ApplicationWorkflow =
  | "installer"
  | "existing"
  | "portable"
  | "winget";

/** A presentation projection only: badges never grant cleanup authority. */
export function applicationRows(
  portable: PortableInstallationView[],
  associated: DetectedApplicationView[],
  winget: WingetInstallationView[],
): ApplicationRow[] {
  const rows = new Map<string, ApplicationRow>();
  for (const item of portable) {
    rows.set(item.installationId, {
      id: item.installationId,
      name: item.applicationName,
      version: item.applicationVersion,
      publisher: null,
      provider: "Portable",
      management:
        item.relationship === "INSTALLED_FOR_CASK" ? "managed" : "associated",
      kind: "portable",
      item,
    });
  }
  for (const item of associated) {
    if (rows.has(item.installationId)) continue;
    rows.set(item.installationId, {
      id: item.installationId,
      name: item.name,
      version: item.displayVersion,
      publisher: item.publisher,
      provider: item.removalPolicy === "MANAGED_MSI" ? "MSI" : "Windows",
      management:
        item.removalPolicy === "MANAGED_MSI" ||
        item.provenance === "INSTALLED_BY_CASKLET"
          ? "managed"
          : "associated",
      kind: "windows",
      item,
    });
  }
  for (const item of winget) {
    const id = item.registryInstallationId || `winget:${item.id}`;
    const observed = rows.get(id);
    if (
      observed?.kind === "portable" ||
      (observed?.kind === "windows" && observed.management === "managed")
    )
      continue;
    rows.set(id, {
      id,
      name: observed?.name ?? item.packageId,
      version: observed?.version ?? item.packageVersion,
      publisher: observed?.publisher ?? null,
      provider: "WinGet",
      management:
        item.provenance === "INSTALLED_BY_CASKLET" && item.state === "INSTALLED"
          ? "managed"
          : "unverified",
      kind: "winget",
      item,
    });
  }
  return [...rows.values()].sort(
    (a, b) =>
      a.name.localeCompare(b.name, undefined, { sensitivity: "base" }) ||
      a.id.localeCompare(b.id),
  );
}
