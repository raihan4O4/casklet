<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { emit } from "@tauri-apps/api/event";
  import type { CaskView } from "../contracts/generated/casklet";
  import { CONTRACT_VERSION } from "../contracts/generated/casklet";
  import CreateCaskForm from "./CreateCaskForm.svelte";
  import CaskBadge from "./CaskBadge.svelte";
  type Request = {
    id: number;
    path: string;
    caskId: string | null;
    batchSize?: number;
  };
  let request: Request | null = $state(null);
  let casks: CaskView[] = $state([]);
  let search = $state("");
  let destination = $state("");
  let error = $state("");
  let busy = $state(false);
  let mode: "create" | "choose" = $state("choose");
  let dialog: HTMLDialogElement;
  let checking = false;
  let refreshing = false;
  let lastRefresh = 0;
  let loadError = $state(false);
  const visible = $derived(
    casks.filter((cask) =>
      cask.name.toLocaleLowerCase().includes(search.trim().toLocaleLowerCase()),
    ),
  );
  async function refresh() {
    if (refreshing) return;
    refreshing = true;
    try {
      const result = await invoke<CaskView[]>("list_casks", {
        request: { version: CONTRACT_VERSION },
      });
      casks = result.filter((cask) => cask.state === "ACTIVE");
      if (destination && !casks.some((cask) => cask.id === destination))
        destination = "";
      if (loadError) error = "";
      loadError = false;
    } catch (cause) {
      loadError = true;
      throw cause;
    } finally {
      lastRefresh = Date.now();
      refreshing = false;
    }
  }
  async function route(id: number, caskId: string) {
    busy = true;
    error = "";
    try {
      await invoke("explorer_route", { id, caskId });
      request = null;
      dialog?.close();
    } catch (cause) {
      error =
        typeof cause === "string"
          ? cause
          : "Unable to open this Cask. Try again or choose another destination.";
    } finally {
      busy = false;
    }
  }
  async function created(cask: CaskView) {
    // Creation never consumes the Explorer request. If routing fails, retry the
    // same request and destination without creating another Cask.
    casks = [...casks.filter((item) => item.id !== cask.id), cask];
    destination = cask.id;
    mode = "choose";
    await emit("casks-changed").catch(() => {});
    if (request) await route(request.id, cask.id);
  }
  async function check() {
    if (checking || refreshing || busy) return;
    if (request) {
      if (mode === "choose" && Date.now() - lastRefresh >= 3000) {
        await refresh()
          .then(async () => {
            if (request && !busy && !casks.length) {
              mode = "create";
              await tick();
              dialog.querySelector<HTMLInputElement>("input")?.focus();
            }
          })
          .catch(() => {
            error = "Unable to load Casks. Retrying automatically.";
          });
      }
      return;
    }
    checking = true;
    try {
      const pending = await invoke<Request | null>("explorer_pending");
      if (
        !pending ||
        typeof pending.id !== "number" ||
        typeof pending.path !== "string"
      )
        return;
      request = pending;
      destination = "";
      search = "";
      error = "";
      mode = "choose";
      await refresh();
      if (!casks.length) {
        mode = "create";
      } else if (
        pending.caskId &&
        casks.some((cask) => cask.id === pending.caskId)
      ) {
        destination = pending.caskId;
        await route(pending.id, destination);
      } else if (pending.caskId) {
        error = "That Cask is no longer available. Choose another destination.";
      }
    } catch {
      if (request) error = "Unable to load Casks. Retrying automatically.";
    } finally {
      checking = false;
      if (request) {
        await tick();
        if (!dialog.open) dialog.showModal();
        dialog.querySelector<HTMLInputElement>("input")?.focus();
      }
    }
  }
  async function cancel() {
    if (!request || busy) return;
    busy = true;
    try {
      await invoke("explorer_ack", { id: request.id });
      request = null;
      dialog.close();
    } catch {
      error = "Unable to dismiss this request. Try again.";
    } finally {
      busy = false;
    }
  }
  onMount(() => {
    void check();
    const timer = window.setInterval(() => void check(), 750);
    return () => window.clearInterval(timer);
  });
</script>

<svelte:window
  onfocus={() => {
    if (request && !busy && !checking && mode === "choose") {
      lastRefresh = 0;
      void check();
    }
  }}
/>
<dialog
  bind:this={dialog}
  aria-labelledby="explorer-title"
  oncancel={(event) => {
    event.preventDefault();
    void cancel();
  }}
>
  {#if mode === "create"}
    <p class="source" title={request?.path}>
      {request?.path.split(/[\\/]/).pop()}
    </p>
    <CreateCaskForm
      headingId="explorer-title"
      inputId="explorer-cask-name"
      description={(request?.batchSize ?? 1) > 1
        ? `Create a Cask to continue reviewing all ${request?.batchSize} selected items.`
        : "Create a Cask to continue adding your selected item. You’ll choose ownership or reference next."}
      bind:busy
      onCreated={created}
      onCancel={() => void cancel()}
    />
    {#if error}<p role="alert">{error}</p>{/if}
  {:else}
    <form
      onsubmit={(event) => {
        event.preventDefault();
        if (request && destination && !busy)
          void route(request.id, destination);
      }}
    >
      <h2 id="explorer-title">Choose a Cask</h2>
      <p class="source" title={request?.path}>
        {request?.path.split(/[\\/]/).pop()}
      </p>
      <p>
        {(request?.batchSize ?? 1) > 1
          ? `Choose where to review all ${request?.batchSize} selected items.`
          : "Choose where to review this item."}
      </p>
      {#if error}<p role="alert">{error}</p>{/if}
      <label for="explorer-search">Search Casks</label>
      <input
        id="explorer-search"
        type="search"
        bind:value={search}
        disabled={busy}
      />
      <fieldset disabled={busy}>
        <legend>Destination</legend>
        <div class="destinations">
          {#each visible as cask (cask.id)}
            <label class:chosen={destination === cask.id}>
              <input
                type="radio"
                name="explorer-destination"
                value={cask.id}
                bind:group={destination}
              />
              <CaskBadge caskId={cask.id} size={30} /><strong
                >{cask.name}</strong
              >
            </label>
          {:else}<p>
              {casks.length
                ? "No matching Casks."
                : loadError
                  ? "Loading available Casks…"
                  : "No Casks are available."}
            </p>{/each}
        </div>
      </fieldset>
      {#if destination}<p>
          Selected: <strong
            >{casks.find((cask) => cask.id === destination)?.name ??
              "Unavailable Cask"}</strong
          >
        </p>{/if}
      <div class="actions">
        <button type="button" disabled={busy} onclick={() => void cancel()}
          >Cancel</button
        >
        {#if !casks.length && !loadError}<button
            type="button"
            disabled={busy}
            onclick={() => {
              mode = "create";
              void tick().then(() =>
                dialog.querySelector<HTMLInputElement>("input")?.focus(),
              );
            }}>Create Cask</button
          >{/if}
        <button
          class="primary-button"
          type="submit"
          disabled={busy || !destination}
          >{busy ? "Opening…" : "Continue"}</button
        >
      </div>
    </form>
  {/if}
</dialog>

<style>
  dialog {
    width: 480px;
    max-width: calc(100vw - 48px);
    padding: 24px;
    border-radius: 10px;
    border: 1px solid var(--border-control, #ddd);
    color: var(--text-primary, #171717);
    background: var(--surface-panel, #fff);
  }
  dialog::backdrop {
    background: #0006;
  }
  h2 {
    margin: 0 0 12px;
  }
  p {
    margin-bottom: 12px;
  }
  .source {
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  input[type="search"] {
    width: 100%;
    margin: 8px 0 16px;
  }
  fieldset {
    border: 0;
    padding: 0;
  }
  .destinations {
    max-height: 260px;
    overflow: auto;
  }
  .destinations label {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px;
    border: 1px solid var(--border, #ddd);
    border-radius: 6px;
    margin: 6px 0;
    cursor: pointer;
  }
  .destinations label.chosen {
    outline: 2px solid var(--focus-border, #555);
    outline-offset: -2px;
  }
  .destinations input {
    width: auto;
    flex: none;
  }
  .destinations strong {
    overflow-wrap: anywhere;
    min-width: 0;
  }
  .destinations label:focus-within {
    outline: 2px solid var(--focus-border, #555);
    outline-offset: -2px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 20px;
  }
  [role="alert"] {
    color: var(--danger-text, #b42318);
  }
</style>
