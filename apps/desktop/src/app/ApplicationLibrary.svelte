<script lang="ts">
  import { slidingSelection } from "./slidingSelection";
  import { tick, type Snippet } from "svelte";
  import Icon from "./Icon.svelte";
  import ApplicationIcon from "./ApplicationIcon.svelte";
  import { bytes } from "./format";
  import type { ApplicationRow, ApplicationWorkflow } from "./applications";
  let {
    rows,
    workflow,
    busy = false,
    needsAttention = false,
    installationRunning = false,
    onWorkflow,
    onAction,
    onLaunch,
    children,
  }: {
    rows: ApplicationRow[];
    workflow: ApplicationWorkflow | null;
    busy?: boolean;
    needsAttention?: boolean;
    installationRunning?: boolean;
    onWorkflow: (value: ApplicationWorkflow | null) => void;
    onAction: (row: ApplicationRow) => void;
    onLaunch: (row: ApplicationRow) => void;
    children: Snippet;
  } = $props();
  const workflows = [
    {
      key: "installer",
      title: "Install Application",
      description: "Choose an MSI or EXE installer",
      icon: "download",
    },
    {
      key: "existing",
      title: "Associate existing app",
      description: "Add an app already on this PC",
      icon: "associate",
    },
    {
      key: "portable",
      title: "Install portable ZIP",
      description: "Keep a portable app in this Cask",
      icon: "package",
    },
    {
      key: "winget",
      title: "Install with WinGet",
      description: "Find an app in the package catalog",
      icon: "search",
    },
  ] as const;
  let addOpen = $state(false);
  let query = $state("");
  let filter: "all" | "managed" | "associated" = $state("all");
  let addButton = $state<HTMLButtonElement>();
  let picker = $state<HTMLDivElement>();
  let workflowPanel = $state<HTMLDivElement>();
  const visible = $derived(
    rows.filter(
      (row) =>
        (filter === "all" || row.management === filter) &&
        [row.name, row.publisher ?? "", row.provider]
          .join(" ")
          .toLocaleLowerCase()
          .includes(query.trim().toLocaleLowerCase()),
    ),
  );

  function reveal(element: HTMLElement | undefined) {
    element?.scrollIntoView?.({
      block: "start",
      behavior: matchMedia("(prefers-reduced-motion: reduce)").matches
        ? "instant"
        : "smooth",
    });
  }
  async function openAdd() {
    addOpen = !addOpen;
    await tick();
    if (addOpen) {
      reveal(picker);
      picker
        ?.querySelector<HTMLButtonElement>("button")
        ?.focus({ preventScroll: true });
    }
  }
  async function choose(value: ApplicationWorkflow) {
    addOpen = false;
    onWorkflow(value);
    await tick();
    reveal(workflowPanel);
    workflowPanel?.focus({ preventScroll: true });
  }
  async function closeWorkflow() {
    onWorkflow(null);
    await tick();
    addButton?.focus({ preventScroll: true });
  }
</script>

<section class="application-section" aria-label="Cask applications">
  <div class="applications-toolbar">
    <div class="applications-heading">
      <span class="applications-section-icon"
        ><Icon name="grid" size={26} /></span
      >
      <div>
        <h2>Applications <span class="count">{rows.length}</span></h2>
        <p>Installed and linked apps.</p>
      </div>
      <button
        bind:this={addButton}
        class="primary"
        type="button"
        disabled={busy}
        aria-expanded={addOpen}
        aria-controls="application-add-picker"
        onclick={() => void openAdd()}
        ><Icon name="plus" size={16} />Add Application</button
      >
    </div>
    {#if rows.length}
      <div class="applications-tools">
        <label class="search-field"
          ><Icon name="search" size={16} /><input
            aria-label="Search Cask applications"
            placeholder="Search apps or publishers…"
            bind:value={query}
          />
          {#if query}<button
              class="icon-button"
              type="button"
              aria-label="Clear app search"
              onclick={() => {
                query = "";
              }}><Icon name="close" size={14} /></button
            >{/if}
        </label>
        <div
          class="application-filters"
          use:slidingSelection={{
            key: filter,
            selector: 'button[aria-pressed="true"]',
          }}
          role="group"
          aria-label="Filter applications"
        >
          {#each [{ key: "all", label: "All" }, { key: "managed", label: "Owned" }, { key: "associated", label: "References" }] as option (option.key)}
            <button
              type="button"
              class:chosen={filter === option.key}
              aria-pressed={filter === option.key}
              onclick={() => {
                filter = option.key as typeof filter;
              }}>{option.label}</button
            >
          {/each}
        </div>
      </div>
    {/if}
  </div>
  {#if needsAttention && workflow !== "installer"}<div
      class="application-attention"
    >
      <Icon name="info" size={16} /><span
        >{installationRunning
          ? "An installation is in progress."
          : "An installation needs attention."}</span
      ><button type="button" onclick={() => void choose("installer")}
        >View installation</button
      >
    </div>{/if}
  {#if addOpen}<div
      bind:this={picker}
      id="application-add-picker"
      class="application-add-picker"
    >
      {#snippet method(option: (typeof workflows)[number])}
        <button
          type="button"
          disabled={busy}
          onclick={() => void choose(option.key)}
        >
          <span class="application-method-icon"
            ><Icon name={option.icon} size={26} /></span
          >
          <span class="application-method-copy"
            ><strong>{option.title}</strong><span>{option.description}</span
            ></span
          >
          <span class="application-method-chevron"
            ><Icon name="right" size={19} /></span
          >
        </button>
      {/snippet}
      <h3>How would you like to add an app?</h3>
      <p class="application-method-caption">
        Choose a method to add an application to this Cask.
      </p>
      <div class="workflow-options">
        {#each workflows.filter((option) => option.key !== "winget") as option (option.key)}
          {@render method(option)}
        {/each}
      </div>
      <details class="application-more-methods">
        <summary>More install options</summary>
        <div class="workflow-options">{@render method(workflows[3])}</div>
      </details>
    </div>{/if}
  {#if workflow}<div
      bind:this={workflowPanel}
      class="application-workflow-panel"
      tabindex="-1"
    >
      <div class="section-heading">
        <h3>{workflows.find((item) => item.key === workflow)?.title}</h3>
        <button
          class="icon-button"
          type="button"
          aria-label="Close application workflow"
          disabled={busy}
          onclick={() => void closeWorkflow()}
          ><Icon name="close" size={16} /></button
        >
      </div>
      {@render children()}
    </div>{/if}
  {#if rows.length === 0}<div class="empty-state application-empty">
      <Icon name="package" size={32} />
      <h3>Your apps, together</h3>
      <p>
        Install an app or link one already on your PC using Add Application
        above.
      </p>
    </div>
  {:else}
    <div class="application-results-heading">
      <h3>Your applications</h3>
      <span role="status"
        >{visible.length === rows.length
          ? `${rows.length} ${rows.length === 1 ? "app" : "apps"}`
          : `${visible.length} of ${rows.length} apps`}</span
      >
    </div>
    {#if visible.length === 0}<div class="empty-state compact">
        <Icon name="search" size={24} />
        <h3>No matching applications</h3>
        <p>Try another name or change the filter.</p>
        <button
          type="button"
          onclick={() => {
            query = "";
            filter = "all";
          }}>Reset filters</button
        >
      </div>
    {:else}<ul
        class="application-list managed-application-list"
        aria-label="Application list"
      >
        {#each visible as row (row.id)}<li>
            <ApplicationIcon
              source={row.iconSource}
              fallback={row.management === "associated" ? "link" : "package"}
            />
            <div class="item-text">
              <strong title={row.name}>{row.name}</strong><span
                >{[row.version, row.publisher, row.provider]
                  .filter(Boolean)
                  .join(" · ")}</span
              >
              <div class="application-row-meta">
                <small class="application-size"
                  >{row.sizeBytes == null
                    ? "Size not measured"
                    : `${row.sizeEstimated ? "Estimated " : ""}${bytes(row.sizeBytes)}`}</small
                >
                <span
                  class="application-badge"
                  class:owned={row.management === "managed"}
                  >{row.management === "managed"
                    ? "Casklet-owned"
                    : row.management === "associated"
                      ? "Reference"
                      : "Not verified"}</span
                >
              </div>
            </div>
            <div class="application-row-actions">
              {#if row.kind === "portable"}<button
                  type="button"
                  aria-label={`Open ${row.name}`}
                  disabled={busy || !row.item.canLaunch}
                  title={!row.item.canLaunch
                    ? "This app is currently unavailable"
                    : "Open app"}
                  onclick={() => onLaunch(row)}
                  ><Icon name="open" size={14} />Open</button
                >{/if}
              <button
                type="button"
                aria-label={`${row.kind === "windows" && !["MANAGED_MSI", "CASKLET_OWNED"].includes(row.item.removalPolicy) ? "Keep app and remove link to" : row.kind === "winget" ? "Manage" : "Remove"} ${row.name}`}
                disabled={busy ||
                  (row.kind === "portable" && !row.item.canPlanRemoval) ||
                  (row.kind === "winget" && !row.item.canPlanUninstall)}
                onclick={() => onAction(row)}
                ><Icon
                  name={row.management === "associated"
                    ? "associate"
                    : "settings"}
                  size={16}
                />{row.kind === "windows" &&
                !["MANAGED_MSI", "CASKLET_OWNED"].includes(
                  row.item.removalPolicy,
                )
                  ? row.management === "managed"
                    ? "Keep app"
                    : "Remove link"
                  : row.kind === "winget"
                    ? "Manage"
                    : "Remove"}</button
              >
            </div>
          </li>{/each}
      </ul>{/if}
  {/if}
</section>
