// Test-only Tauri transport fixture. Imported by ui-audit, never by the app.
export function installFixture(options = {}) {
  const now = Date.now();
  const gib = 1024 ** 3;
  const names = [
    "Photography Course",
    "Game Modding",
    "Client Project",
    "MCA Study",
    "Personal Tools",
  ];
  const descriptions = [
    "Course files, practice photos and related resources",
    "Game mods, tools, textures and documentation",
    "Design files, source code and client applications",
    "Study notes, projects and related software",
    "Utilities, portable apps and personal resources",
  ];
  let casks = names.map((name, i) => ({
    version: 1,
    id: `qa-cask-${i}`,
    name,
    description: descriptions[i],
    state: "ACTIVE",
    managedStorageRoot: `D:\\Casklets\\qa-cask-${i}`,
    createdAtMs: now - (90 + i) * 86400000,
    updatedAtMs: now - [2, 7, 14, 21, 30][i] * 86400000,
    revision: 0,
  }));
  if (options.empty) casks = [];
  if (options.long) {
    casks[0].name =
      "Photography Course: field studies, practical assignments and an exceptionally long project title";
    casks[0].description =
      "A deliberately long resource description with non-ASCII characters: 資料, études and photography practice.";
  }
  const resources = [
    ["Course notes.pdf", "FILE", "documents", "MANAGED", 13 * 1024 ** 2],
    [
      "Practice photos",
      "DIRECTORY",
      "images",
      "MANAGED",
      12.4 * gib - 13 * 1024 ** 2,
    ],
    [
      "Editing references",
      "DIRECTORY",
      "images",
      "REFERENCED",
      1.4 * gib - 44000,
    ],
    ["Project brief.docx", "FILE", "documents", "REFERENCED", 44000],
  ].map(([name, kind, categoryKey, management, sizeBytes], i) => ({
    version: 1,
    id: `resource-${i}`,
    caskId: "qa-cask-0",
    name,
    path: `D:\\Projects\\Photography Course\\Resources\\${name}`,
    kind,
    categoryKey,
    management,
    sizeBytes,
    sizeMeasurement: "LOGICAL",
    unknownEntries: 0,
    shared: i === 2,
    containment: management === "MANAGED" ? "INTERNAL" : "EXTERNAL",
    availability: "PRESENT",
    ownership: management === "MANAGED" ? "OWNED" : "REFERENCED",
    provenance: "USER_REFERENCE",
  }));
  const apps = ["Adobe Lightroom", "Blender"].map((name, i) => ({
    version: 1,
    applicationId: `app-${i}`,
    installationId: `installation-${i}`,
    name,
    publisher: i ? "Blender Foundation" : "Adobe Inc.",
    displayVersion: i ? "4.5" : "15.0",
    scope: "USER",
    sourceKey: `HKCU\\Software\\${name}`,
    installLocation: `C:\\Applications\\${name}`,
    state: "PRESENT",
    ambiguousName: false,
    associatedCaskId: "qa-cask-0",
    provenance: "USER_ASSOCIATION",
    removalPolicy: "DETACH_ONLY",
  }));
  const operations = [
    "REGISTER_RESOURCE",
    "INSTALL_APPLICATION",
    "EXPORT_CASK",
  ].map((step, i) => ({
    version: 1,
    id: `operation-${i}`,
    caskId: "qa-cask-0",
    state: "COMPLETE",
    step,
    progressCompleted: 1,
    progressTotal: 1,
    cancellationRequested: false,
    errorCode: null,
    updatedAtMs: now - [3600000, 7200000, 86400000][i],
  }));
  const recovery = { version: 1, operations: [], pendingManifestCasks: [] };
  const categories = ["documents", "images", "applications", "other"].map(
    (key) => ({
      version: 1,
      key,
      name: key[0].toUpperCase() + key.slice(1),
      isBuiltin: true,
    }),
  );
  const callbackMap = new Map();
  let cleanupPlan = null;
  let nextId = 1;
  window.__QA_CALLS__ = [];
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
  window.__TAURI_INTERNALS__ = {
    metadata: {
      currentWindow: { label: "main" },
      currentWebview: { label: "main" },
    },
    transformCallback(callback) {
      const id = nextId++;
      callbackMap.set(id, callback);
      return id;
    },
    unregisterCallback(id) {
      callbackMap.delete(id);
    },
    convertFileSrc(path) {
      return path;
    },
    async invoke(command, args = {}) {
      window.__QA_CALLS__.push({ command, args });
      if (command === "plugin:event|listen") return nextId++;
      if (command === "plugin:window|title")
        return new URLSearchParams(location.search).has("cask")
          ? `${casks[0]?.name ?? "Casklet"} · Casklet`
          : "Casklet";
      if (command.startsWith("plugin:dialog|open"))
        return "D:\\Projects\\Photography Course\\Course notes.pdf";
      if (command.startsWith("plugin:")) return null;
      if (command === "cask_library") {
        if (options.failed)
          throw { message: "The local library is temporarily unavailable." };
        const summaries = casks.map((cask, i) => ({
          cask,
          itemCount: [6, 42, 21, 18, 9][i],
          fileCount: [4, 40, 18, 16, 6][i],
          applicationCount: [2, 2, 3, 2, 3][i],
          managedBytes: [12.4, 22.4, 11.8, 7.7, 18.2][i] * gib,
          referencedBytes: [6.2, 1.9, 0.9, 0.4, 0.5][i] * gib,
          unknownEntries: 0,
        }));
        return {
          version: 1,
          casks: summaries,
          totalItems: casks.length ? 96 : 0,
          managedBytes: casks.length ? 72.5 * gib : 0,
          referencedBytes: casks.length ? 9.9 * gib : 0,
          unknownEntries: 0,
        };
      }
      if (command === "list_casks") return casks;
      if (command === "get_cask")
        return casks.find((cask) => cask.id === args.request.id) ?? casks[0];
      if (command === "create_cask") {
        const cask = {
          ...casks[0],
          version: 1,
          id: `qa-new-${nextId++}`,
          name: args.request.name,
          description: "",
          state: "ACTIVE",
          createdAtMs: now,
          updatedAtMs: now,
          managedStorageRoot: "D:\\Casklets\\qa-new",
          revision: 0,
        };
        casks.push(cask);
        return cask;
      }
      if (command === "recovery_status") return recovery;
      if (command === "list_operations") return options.empty ? [] : operations;
      if (command === "list_resources") return options.empty ? [] : resources;
      if (command === "list_categories") return categories;
      if (command === "categorize_resource") {
        const item = resources.find(
          (item) => item.id === args.request.resourceId,
        );
        item.categoryKey = args.request.categoryKey;
        return { ...item };
      }
      if (command === "set_resource_ownership") {
        const item = resources.find(
          (item) => item.id === args.request.resourceId,
        );
        item.ownership = args.request.owned ? "OWNED" : "REFERENCED";
        item.management = args.request.owned ? "MANAGED" : "REFERENCED";
        return { ...item };
      }
      if (command === "create_category") {
        const category = {
          version: 1,
          key: "qa-custom",
          name: args.request.name,
          isBuiltin: false,
        };
        categories.push(category);
        return category;
      }
      if (command === "resource_contents")
        return {
          version: 1,
          resourceId: args.request.id,
          ownership: "REFERENCED",
          availability: "PRESENT",
          totalEntries: 2,
          unknownEntries: 0,
          entries: [
            { relativePath: "reference.jpg", kind: "FILE", sizeBytes: 102400 },
            { relativePath: "notes.txt", kind: "FILE", sizeBytes: 2048 },
          ],
        };
      if (
        command === "list_associated_applications" ||
        command === "search_installed_applications"
      )
        return apps;
      if (command === "storage_summary")
        return {
          version: 1,
          managedLogicalBytes: 12.4 * gib,
          referencedLogicalBytes: 1.4 * gib,
          ownedApplicationBytes: 0,
          referenceApplicationBytes: 4.8 * gib,
          estimatedApplications: 0,
          applicationSizes: apps.map((app) => ({
            sourceKey: app.sourceKey,
            sizeBytes: 2.4 * gib,
            estimated: false,
          })),
          physicalBytes: null,
          reclaimableMinBytes: 0,
          reclaimableMaxBytes: 0,
          unknownEntries: 0,
        };
      if (
        command === "list_portable_installations" ||
        command === "list_winget_installations" ||
        command === "list_installer_artifacts" ||
        command === "list_installer_operations" ||
        command === "list_removal_plans"
      )
        return [];
      if (command === "cask_removal_status")
        return {
          version: 1,
          canRemove: false,
          resourceCount: 4,
          applicationCount: 2,
          unfinishedOperations: 0,
        };
      if (command === "removal_capability") return !!options.removalEnabled;
      if (command === "create_removal_plan") {
        cleanupPlan = {
          version: 1,
          id: "qa-plan",
          caskId: "qa-cask-0",
          state: "DRAFT",
          scope: "CATEGORY",
          planHash: "qa-exact-preview-hash",
          permanent: !!args.request.permanent,
          cleanupOnly: true,
          createdAtMs: now,
          expiresAtMs: now + 600000,
          items: resources
            .filter((item) => item.categoryKey === args.request.categoryKey)
            .map((item) => ({
              id: `plan-${item.id}`,
              resourceId: item.id,
              name: item.name,
              path: item.path,
              action:
                item.management === "REFERENCED"
                  ? "DETACH"
                  : args.request.permanent
                    ? "DELETE_PERMANENTLY"
                    : "TRASH",
              reasonCode:
                item.management === "REFERENCED"
                  ? "NO_DELETE_AUTHORITY"
                  : "ELIGIBLE_MANAGED",
              state: "PENDING",
              expectedSizeBytes: item.sizeBytes,
              resultCode: null,
              recoveryPath: null,
            })),
        };
        return cleanupPlan;
      }
      if (command === "approve_removal_plan") {
        cleanupPlan = { ...cleanupPlan, state: "APPROVED" };
        return cleanupPlan;
      }
      if (command === "get_removal_plan") return cleanupPlan;
      if (
        command === "pending_whole_cask_cleanup" ||
        command === "explorer_pending" ||
        command === "latest_msi_cleanup"
      )
        return null;
      if (command === "winget_capabilities")
        return { available: false, canInstall: false, canUninstall: false };
      if (
        command === "set_window_theme" ||
        command === "window_ready" ||
        command === "open_cask_window"
      )
        return null;
      throw new Error(`Unhandled QA IPC command: ${command}`);
    },
  };
  localStorage.setItem("casklet.appearance", options.theme ?? "light");
}
