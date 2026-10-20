<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, tick } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import type {
    ApiError,
    CaskLibraryView,
    CaskSummaryView,
    CaskView,
    DetectedApplicationView,
    RecoveryStatus,
  } from "../contracts/generated/casklet";
  import { CONTRACT_VERSION } from "../contracts/generated/casklet";
  import BrandLogo from "./BrandLogo.svelte";
  import CreateCaskForm from "./CreateCaskForm.svelte";
  import Icon from "./Icon.svelte";
  import Workspace from "./Workspace.svelte";
  // Cask row actions intentionally hidden for Alpha; typed handlers retained.
  import { notify } from "./notifications";
  let { onReady = () => {} }: { onReady?: () => void } = $props();
  $effect(() => {
    if (error) {
      notify(error, "error", "main-error");
      error = "";
    }
  });
  $effect(() => {
    if (notice) {
      notify(notice);
      notice = "";
    }
  });
  import { caskColor, bytes, relativeTime, label } from "./format";

  let page: "casks" | "installations" | "activity" | "settings" =
    $state("casks");
  let library: CaskLibraryView | null = $state(null);
  // Match the per-Cask totals, including references. This is logical size
  // across associations, not physical usage or reclaimable space.
  let usedStorageBytes = $derived.by(() =>
    library?.casks.reduce(
      (total, item) => total + item.managedBytes + item.referencedBytes,
      0,
    ),
  );
  let loading = $state(true);
  let refreshSequence = 0;
  let error = $state("");
  let notice = $state("");
  let search = $state("");
  let creating = $state(false);
  let creationBusy = $state(false);
  let recoveryCount = $state(0);
  let activeCask = $state("");
  let sort: "name" | "items" | "storage" | "updated" = $state("updated");
  let ascending = $state(false);
  let createDialog = $state<HTMLDialogElement>(null!);
  let installedQuery = $state("");
  let installations: DetectedApplicationView[] = $state([]);
  let installationsBusy = $state(false);
  let inventorySearched = $state(false);
  let visibleCasks = $derived.by(() => {
    const items = (library?.casks ?? []).filter(({ cask }) =>
      `${cask.name} ${cask.description}`
        .toLocaleLowerCase()
        .includes(search.trim().toLocaleLowerCase()),
    );
    return items.sort((a, b) => {
      const result =
        sort === "name"
          ? a.cask.name.localeCompare(b.cask.name)
          : sort === "items"
            ? a.itemCount - b.itemCount
            : sort === "storage"
              ? a.managedBytes +
                a.referencedBytes -
                (b.managedBytes + b.referencedBytes)
              : a.cask.updatedAtMs - b.cask.updatedAtMs;
      return (
        (ascending ? result : -result) || a.cask.id.localeCompare(b.cask.id)
      );
    });
  });
  function message(cause: unknown) {
    return typeof cause === "object" && cause !== null && "message" in cause
      ? String((cause as ApiError).message)
      : String(cause);
  }
  async function refresh(quiet = false) {
    const sequence = ++refreshSequence;
    try {
      const result = await invoke<CaskLibraryView>("cask_library", {
        request: { version: CONTRACT_VERSION },
      });
      if (sequence !== refreshSequence) return;
      library = result;
      if (!quiet) error = "";
    } catch (cause) {
      if (!quiet) error = message(cause);
    } finally {
      loading = false;
    }
  }
  async function openCask(id: string) {
    try {
      await invoke("open_cask_window", {
        request: { version: CONTRACT_VERSION, id },
      });
      activeCask = id;
      error = "";
    } catch (cause) {
      error = message(cause);
    }
  }
  async function openStorage(id?: string) {
    try {
      await invoke(id ? "open_cask_storage" : "open_storage_root", {
        request: { version: CONTRACT_VERSION, ...(id ? { id } : {}) },
      });
    } catch (cause) {
      error = message(cause);
    }
  }
  // Retained while the Cask row menu is intentionally hidden for Alpha.
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  async function exportReport(item: CaskSummaryView) {
    try {
      const outputPath = await save({ defaultPath: `${item.cask.name}.md` });
      if (outputPath) {
        await invoke("export_cask", {
          caskId: item.cask.id,
          outputPath,
          format: "markdown",
          machine: false,
        });
        notice = "Cask report exported.";
      }
    } catch (cause) {
      error = message(cause);
    }
  }
  function changeSort(next: typeof sort) {
    if (sort === next) ascending = !ascending;
    else {
      sort = next;
      ascending = true;
    }
  }
  async function showCreate() {
    creating = true;
    await tick();
    createDialog.showModal();
    document.getElementById("cask-name")?.focus();
  }
  function closeCreate() {
    if (creationBusy) return;
    createDialog.close();
    creating = false;
  }
  async function created(cask: CaskView) {
    createDialog.close();
    creating = false;
    notice = "Cask created.";
    await refresh();
    await openCask(cask.id);
  }
  async function searchInstalled(refreshInventory = false) {
    installationsBusy = true;
    try {
      if (refreshInventory)
        await invoke("refresh_installed_applications", {
          request: { version: CONTRACT_VERSION },
        });
      installations = await invoke<DetectedApplicationView[]>(
        "search_installed_applications",
        { request: { version: CONTRACT_VERSION, query: installedQuery } },
      );
      inventorySearched = true;
      error = "";
    } catch (cause) {
      error = message(cause);
    } finally {
      installationsBusy = false;
    }
  }
  onMount(() => {
    void refresh().then(onReady);
    const poll = window.setInterval(() => {
      void refresh(true);
      void invoke<RecoveryStatus>("recovery_status")
        .then((value) => {
          recoveryCount =
            value.operations.length + value.pendingManifestCasks.length;
        })
        .catch(() => {});
    }, 5000);
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let unlistenRemoved: (() => void) | undefined;
    void listen<string>("casks-removed", (event) => {
      ++refreshSequence;
      if (library)
        library = {
          ...library,
          casks: library.casks.filter((item) => item.cask.id !== event.payload),
        };
      if (activeCask === event.payload) activeCask = "";
      void refresh(true);
    }).then((fn) => {
      if (disposed) fn();
      else unlistenRemoved = fn;
    });
    void listen("casks-changed", () => {
      void refresh(true);
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      window.clearInterval(poll);
      unlisten?.();
      unlistenRemoved?.();
    };
  });
</script>

<a class="skip-link" href="#main-content">Skip to main content</a>
<div class="main-window">
  <aside class="sidebar" aria-label="Main navigation">
    <div class="brand">
      <BrandLogo /><span>Casklet</span>
    </div>
    <nav aria-label="Main sections">
      {#each [{ key: "casks", title: "Casks" }, { key: "installations", title: "Installations" }, { key: "activity", title: "Activity" }, { key: "settings", title: "Settings" }] as item (item.key)}
        <button
          type="button"
          class:active={page === item.key}
          aria-current={page === item.key ? "page" : undefined}
          onclick={() => {
            page = item.key as typeof page;
            if (page === "installations" && !inventorySearched)
              void searchInstalled();
          }}
          ><Icon
            name={item.key as
              | "casks"
              | "installations"
              | "activity"
              | "settings"}
          />{item.title}{#if item.key === "activity" && recoveryCount}<span
              class="nav-attention"
              title="Recovery needs attention">{recoveryCount}</span
            >{/if}</button
        >
      {/each}
    </nav>
    <div class="sidebar-storage">
      <span class="muted">Managed storage</span><strong
        >{bytes(library?.managedBytes)}</strong
      >
      <p>Known logical size{library?.unknownEntries ? " · incomplete" : ""}</p>
      <button
        class="text-button"
        type="button"
        onclick={() => void openStorage()}
        ><Icon name="folder" size={16} />Open Storage Folder</button
      >
    </div>
  </aside>
  <main id="main-content" class="main-content">
    <header class="page-header">
      <div>
        <h1>
          {page === "casks"
            ? "Casks"
            : page === "installations"
              ? "Installations"
              : page === "activity"
                ? "Activity"
                : "Settings"}
        </h1>
        <p>
          {page === "casks"
            ? "Keep everything for a project together."
            : page === "installations"
              ? "Discover the applications installed on this computer."
              : page === "activity"
                ? "Operations, history and recovery."
                : "Your local workspace and preferences."}
        </p>
      </div>
      {#if page === "casks"}<div class="header-actions">
          <label class="search-field"
            ><Icon name="search" size={17} /><input
              aria-label="Search casks"
              bind:value={search}
              placeholder="Search casks…"
            /></label
          ><button
            class="primary-button"
            type="button"
            onclick={() => void showCreate()}
            ><Icon name="plus" size={17} />New Cask</button
          >
        </div>{/if}
    </header>
    {#if page === "casks"}
      <div class="summary-cards">
        <div class="summary-card">
          <span class="summary-icon"><Icon name="casks" size={22} /></span>
          <div>
            <strong>{library?.casks.length ?? "—"}</strong><span
              >Total Casks</span
            >
          </div>
        </div>
        <div class="summary-card">
          <span class="summary-icon"><Icon name="storage" size={22} /></span>
          <div>
            <strong>{bytes(usedStorageBytes)}</strong><span>Used Storage</span
            ><small
              title="Sum of known Cask sizes, including references. Content associated with multiple Casks is counted in each. This is not physical disk usage or reclaimable space."
              >Includes references{library?.unknownEntries
                ? " · incomplete"
                : ""}</small
            >
          </div>
        </div>
        <div class="summary-card">
          <span class="summary-icon"><Icon name="file" size={22} /></span>
          <div>
            <strong>{library?.totalItems ?? "—"}</strong><span>Total Items</span
            ><small>Files, folders and apps</small>
          </div>
        </div>
      </div>
      <section class="cask-table" aria-label="Casks">
        <div class="table-heading">
          {#each [{ key: "name", title: "Name" }, { key: "items", title: "Items" }, { key: "storage", title: "Storage" }, { key: "updated", title: "Updated" }] as column (column.key)}
            <button
              type="button"
              class:active={sort === column.key}
              aria-pressed={sort === column.key}
              title={sort === column.key
                ? `Sorted ${ascending ? "ascending" : "descending"}. Click to reverse.`
                : `Sort by ${column.title.toLowerCase()}`}
              data-direction={sort === column.key
                ? ascending
                  ? "ascending"
                  : "descending"
                : undefined}
              onclick={() => changeSort(column.key as typeof sort)}
              >{column.title}{#if sort === column.key}<Icon
                  name={ascending ? "up" : "down"}
                  size={13}
                />{/if}</button
            >
          {/each}
        </div>
        <div class="cask-table-scroll">
          {#if loading}<div class="empty-state" role="status">
              <p>Loading Casks…</p>
            </div>{:else if !library}<div class="empty-state">
              <h2>Couldn’t load your Casks</h2>
              <button type="button" onclick={() => void refresh()}
                >Try again</button
              >
            </div>{:else if library.casks.length === 0}<div class="empty-state">
              <Icon name="casks" size={30} />
              <h2>No Casks yet</h2>
              <p>
                Create a Cask for a project, course or activity.<br />Add files
                and applications when you’re ready.
              </p>
              <button
                class="primary-button"
                type="button"
                onclick={() => void showCreate()}
                ><Icon name="plus" />Create your first Cask</button
              >
            </div>{:else if visibleCasks.length === 0}<div class="empty-state">
              <h2>No matching Casks</h2>
              <p>Try another name or description.</p>
              <button
                type="button"
                onclick={() => {
                  search = "";
                }}>Clear search</button
              >
            </div>{:else}
            <ul>
              {#each visibleCasks as item (item.cask.id)}<li
                  class:selected={activeCask === item.cask.id}
                >
                  <button
                    type="button"
                    class="cask-row"
                    aria-label={`Open ${item.cask.name}`}
                    onclick={() => void openCask(item.cask.id)}
                    ><div class="cask-name-cell">
                      <span
                        class="status-dot"
                        style:background={item.cask.state === "ACTIVE"
                          ? caskColor(item.cask.id)
                          : "#999"}
                        title={label(item.cask.state)}
                      ></span>
                      <div>
                        <strong title={item.cask.name}>{item.cask.name}</strong>
                        <p title={item.cask.description}>
                          {item.cask.description ||
                            "Files, applications and related resources"}
                        </p>
                      </div>
                    </div>
                    <span>{item.itemCount}</span><span
                      title="Known logical size including references"
                      >{bytes(
                        item.managedBytes + item.referencedBytes,
                      )}{item.unknownEntries ? " +" : ""}</span
                    ><time
                      title={new Date(item.cask.updatedAtMs).toLocaleString()}
                      >{relativeTime(item.cask.updatedAtMs)}</time
                    ></button
                  >
                </li>{/each}
            </ul>
          {/if}
        </div>
      </section>
    {:else if page === "installations"}
      <div class="inventory-toolbar">
        <form
          class="search-field"
          onsubmit={(event) => {
            event.preventDefault();
            void searchInstalled();
          }}
        >
          <Icon name="search" size={17} /><input
            aria-label="Search installed applications"
            placeholder="Search applications…"
            bind:value={installedQuery}
          /><button class="text-button" disabled={installationsBusy}
            >Search</button
          >
        </form>
        <button
          type="button"
          disabled={installationsBusy}
          onclick={() => void searchInstalled(true)}
          ><Icon name="refresh" size={16} />Refresh inventory</button
        >
      </div>
      <p class="footnote">
        Detected installations are references. Open a Cask to associate an
        application with it.
      </p>
      <section
        class="installation-inventory scroll-area"
        aria-label="Installed applications"
      >
        {#if installationsBusy}<p role="status">
            Reading installed applications…
          </p>{:else if !installations.length}<div class="empty-state">
            <Icon name="package" size={30} />
            <h2>No installations to display</h2>
            <p>Refresh the Windows inventory or try another search.</p>
          </div>{:else}<ul class="application-list">
            {#each installations as item (item.installationId)}<li>
                <Icon name="package" />
                <div class="item-text">
                  <strong title={item.name}>{item.name}</strong><span
                    >{item.publisher ?? "Unknown publisher"} · {item.displayVersion ??
                      "Version not recorded"} · {label(item.scope)}</span
                  ><small
                    class="truncate path"
                    title={item.installLocation ?? item.sourceKey}
                    >{item.installLocation ?? item.sourceKey}</small
                  >
                </div>
                <span class="state-label">{label(item.state)}</span>
              </li>{/each}
          </ul>{/if}
      </section>
    {:else}<Workspace mode={page} />{/if}
  </main>
</div>
{#if creating}<dialog
    bind:this={createDialog}
    oncancel={(event) => {
      event.preventDefault();
      closeCreate();
    }}
    aria-labelledby="create-heading"
  >
    <CreateCaskForm
      onCreated={created}
      onCancel={closeCreate}
      bind:busy={creationBusy}
    />
  </dialog>{/if}
