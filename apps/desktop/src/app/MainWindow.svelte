<script lang="ts">
  import { surfaceFade } from "./motion";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, tick } from "svelte";
  import type {
    ApiError,
    CaskLibraryView,
    CaskView,
    DetectedApplicationView,
    RecoveryStatus,
  } from "../contracts/generated/casklet";
  import { CONTRACT_VERSION } from "../contracts/generated/casklet";
  import NavigationRail from "./NavigationRail.svelte";
  import CreateCaskForm from "./CreateCaskForm.svelte";
  import AssociateApplication from "./AssociateApplication.svelte";
  import { assignCaskColors, caskColors, colorForCask } from "./caskColors";
  import { caskAppearances } from "./caskAppearance";
  import Icon from "./Icon.svelte";
  import CasksIllustration from "./CasksIllustration.svelte";
  import CaskBadge from "./CaskBadge.svelte";
  import ApplicationIcon from "./ApplicationIcon.svelte";
  import Workspace from "./Workspace.svelte";
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
  import { bytes, relativeTime, label } from "./format";

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
  let refreshing = $state(false);
  let libraryError = $state("");
  let refreshSequence = 0;
  let error = $state("");
  let notice = $state("");
  let search = $state("");
  let collectionView: "grid" | "list" = $state("grid");
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
  let associationTarget: DetectedApplicationView | null = $state(null);
  let associationOrigin: HTMLButtonElement | null = null;
  let associationLabels: Record<string, string> = $state({});
  async function closeAssociation() {
    associationTarget = null;
    await tick();
    associationOrigin?.focus();
  }

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
    // A library scan can outlast the polling interval. Never invalidate its
    // result by starting another poll while it is still running.
    if (refreshing) return;
    refreshing = true;
    if (!quiet) {
      loading = !library;
      libraryError = "";
    }
    const sequence = ++refreshSequence;
    try {
      const result = await invoke<CaskLibraryView>("cask_library", {
        request: { version: CONTRACT_VERSION },
      });
      if (sequence !== refreshSequence) return;
      assignCaskColors(result.casks.map((item) => item.cask));
      library = result;
      libraryError = "";
      if (!quiet) error = "";
    } catch (cause) {
      if (sequence !== refreshSequence) return;
      if (!library || !quiet) libraryError = message(cause);
      if (!quiet && library) error = message(cause);
    } finally {
      loading = false;
      refreshing = false;
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
    // Reveal the rendered loading state; resource/inventory scans must not keep
    // an otherwise ready native window hidden for their entire duration.
    void refresh();
    onReady();
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

<svelte:window
  onkeydown={(event) => {
    if (
      (event.ctrlKey || event.metaKey) &&
      event.key.toLowerCase() === "k" &&
      ["casks", "installations"].includes(page) &&
      !document.querySelector("dialog[open]")
    ) {
      event.preventDefault();
      document
        .querySelector<HTMLInputElement>(
          page === "casks"
            ? '[aria-label="Search casks"]'
            : '[aria-label="Search installed applications"]',
        )
        ?.focus();
    }
  }}
/>
<a class="skip-link" href="#main-content">Skip to main content</a>
<div class="main-window">
  <NavigationRail
    label="Main sections"
    current={page}
    items={[
      { key: "casks", title: "Casks", icon: "casks" },
      { key: "installations", title: "Installations", icon: "installations" },
      {
        key: "activity",
        title: "Activity",
        icon: "activity",
        badge: recoveryCount,
      },
      { key: "settings", title: "Settings", icon: "settings" },
    ]}
    onselect={(key) => {
      page = key as typeof page;
      if (page === "installations" && !inventorySearched)
        void searchInstalled();
    }}
  >
    {#snippet footer()}
      <button
        class="text-button"
        type="button"
        onclick={() => void openStorage()}
        title={`Managed storage: ${bytes(library?.managedBytes)}. Saved logical size${library?.unknownEntries ? ", incomplete" : ""}`}
      >
        <Icon name="storage" size={19} /><span>Local storage</span>
      </button>
    {/snippet}
  </NavigationRail>
  <main
    id="main-content"
    class="main-content"
    class:library-page={page === "casks"}
    class:main-settings-page={page === "settings"}
    class:installations-page={page === "installations"}
  >
    <div class="page-intro" class:library-intro={page === "casks"}>
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
              ? "Your activities, and everything that belongs with them."
              : page === "installations"
                ? "Discover the applications installed on this computer."
                : page === "activity"
                  ? "Operations, history and recovery."
                  : "Your local workspace and preferences."}
          </p>
        </div>
      </header>
      {#if page === "casks"}
        <CasksIllustration />
        <dl class="library-summary" aria-label="Library summary">
          <div>
            <dt>
              <span class="summary-icon"><Icon name="layers" size={34} /></span
              >Casks
            </dt>
            <dd>
              {library?.casks.length ?? "—"}<small class="measurement-note"
                >{library?.totalItems ?? "—"}
                {library?.totalItems === 1 ? "item" : "items"}</small
              >
            </dd>
          </div>
          <div
            title="Sum of known Cask sizes, including references. Content associated with multiple Casks is counted in each. This is not physical disk usage or reclaimable space."
          >
            <dt>
              <span class="summary-icon"><Icon name="chart" size={34} /></span
              >Known Cask size
            </dt>
            <dd>
              {bytes(usedStorageBytes)}<small class="measurement-note"
                >Includes references{library?.unknownEntries
                  ? " · incomplete"
                  : ""}</small
              >
            </dd>
          </div>
        </dl>
      {/if}
    </div>
    {#if page === "casks"}
      <div class="cask-toolbar">
        <div class="library-search-actions">
          <label class="search-field"
            ><Icon name="search" size={17} /><input
              aria-label="Search casks"
              bind:value={search}
              placeholder="Search casks…"
            />{#if search}<button
                class="icon-button"
                type="button"
                aria-label="Clear Cask search"
                onclick={() => {
                  search = "";
                }}><Icon name="close" size={14} /></button
              >{/if}<kbd>Ctrl K</kbd></label
          >
          <button
            class="primary-button new-cask-button"
            type="button"
            onclick={() => void showCreate()}
            ><Icon name="plus" size={17} /><span>New Cask</span></button
          >
        </div>
        <span class="list-context" aria-live="polite"
          >{#if !loading && library}{search.trim()
              ? `${visibleCasks.length} of ${library.casks.length}`
              : `${library.casks.length}`}
            {library.casks.length === 1 ? "cask" : "casks"}{/if}</span
        >
        <div class="view-control" role="group" aria-label="Collection layout">
          <button
            type="button"
            aria-label="Card view"
            aria-pressed={collectionView === "grid"}
            onclick={() => {
              collectionView = "grid";
            }}><Icon name="grid" size={16} /></button
          >
          <button
            type="button"
            aria-label="List view"
            aria-pressed={collectionView === "list"}
            onclick={() => {
              collectionView = "list";
            }}><Icon name="list" size={16} /></button
          >
        </div>
      </div>
      <section
        class="cask-table"
        class:collection-grid={collectionView === "grid"}
        aria-label="Casks"
      >
        <div class="table-heading">
          <span class="sort-count">{visibleCasks.length} casks</span>
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
              {#if libraryError}<p role="alert">{libraryError}</p>{/if}
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
                    style:--cask-color={item.cask.state !== "ACTIVE"
                      ? "#999999"
                      : ($caskAppearances[item.cask.id]?.color ??
                        $caskColors[item.cask.id] ??
                        colorForCask(item.cask.id))}
                    aria-label={`Open ${item.cask.name}`}
                    aria-current={activeCask === item.cask.id
                      ? "true"
                      : undefined}
                    onclick={() => void openCask(item.cask.id)}
                    ><svg
                      class="cask-card-wave"
                      viewBox="0 0 180 90"
                      preserveAspectRatio="none"
                      aria-hidden="true"
                    >
                      <path
                        d="M0 78C35 75 49 4 87 4C116 4 126 47 150 34C162 27 172 18 180 22V90H0Z"
                        fill="currentColor"
                        opacity=".045"
                      />
                      <path
                        d="M0 90C56 86 76 88 105 57C131 30 153 13 180 20V90Z"
                        fill="currentColor"
                        opacity=".075"
                      />
                    </svg>
                    <div class="cask-name-cell">
                      <CaskBadge
                        caskId={item.cask.id}
                        muted={item.cask.state !== "ACTIVE"}
                        title={label(item.cask.state)}
                      />
                      <div>
                        <strong title={item.cask.name}>{item.cask.name}</strong>
                        <p title={item.cask.description}>
                          {item.cask.description ||
                            "Add files and applications to this Cask"}
                        </p>
                      </div>
                    </div>
                    <span class="cask-card-divider" aria-hidden="true"></span>
                    <span class="collection-items"
                      ><Icon name="file" size={14} />{item.itemCount}<span
                        class="card-only"
                        >{item.itemCount === 1 ? "item" : "items"}</span
                      ></span
                    ><span
                      class="collection-size"
                      title="Last known logical size including references; refresh sizes inside the Cask to update"
                      ><span class="list-only"
                        ><Icon name="database" size={18} /></span
                      >{bytes(
                        item.managedBytes + item.referencedBytes,
                      )}{item.unknownEntries ? " +" : ""}</span
                    ><time
                      class="collection-updated"
                      title={new Date(item.cask.updatedAtMs).toLocaleString()}
                      ><Icon name="calendar" size={22} /><span
                        >Updated {relativeTime(item.cask.updatedAtMs)}</span
                      ></time
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
          id="installation-search"
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
          /><kbd>Ctrl K</kbd>
        </form>
        <button
          form="installation-search"
          type="submit"
          disabled={installationsBusy}>Search</button
        >
        <button
          type="button"
          disabled={installationsBusy}
          onclick={() => void searchInstalled(true)}
          ><Icon name="refresh" size={16} />Refresh inventory</button
        >
      </div>
      <p class="footnote">
        Associate an installed application with a Cask as a reference.
        Installation and ownership stay unchanged.
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
                <ApplicationIcon
                  source={{ kind: "registry", key: item.sourceKey }}
                />
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
                <div class="installation-association">
                  {#if item.state !== "PRESENT"}<span class="state-label"
                      >{label(item.state)}</span
                    >{/if}
                  {#if associationLabels[item.installationId]}<small
                      role="status"
                      >Associated with {associationLabels[
                        item.installationId
                      ]}</small
                    >{/if}
                  <button
                    type="button"
                    aria-label={`Associate ${item.name}`}
                    onclick={(event) => {
                      associationOrigin = event.currentTarget;
                      associationTarget = item;
                    }}><Icon name="associate" size={15} />Associate</button
                  >
                </div>
              </li>{/each}
          </ul>{/if}
      </section>
    {:else}<Workspace mode={page} />{/if}
  </main>
</div>
{#if creating}<dialog
    in:surfaceFade
    out:surfaceFade
    bind:this={createDialog}
    onoutroend={() => createDialog.close()}
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

{#if associationTarget}<AssociateApplication
    application={associationTarget}
    onClose={() => void closeAssociation()}
    onCreate={() => {
      associationTarget = null;
      void showCreate();
    }}
    onAssociated={(cask) => {
      if (associationTarget)
        associationLabels[associationTarget.installationId] = cask.name;
      notice = `Application associated with ${cask.name} as a reference.`;
      void closeAssociation();
      void refresh(true);
    }}
  />{/if}
