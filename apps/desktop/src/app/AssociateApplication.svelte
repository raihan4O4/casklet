<script lang="ts">
  import { surfaceFade } from "./motion";
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { emit } from "@tauri-apps/api/event";
  import {
    CONTRACT_VERSION,
    type CaskView,
    type DetectedApplicationView,
  } from "../contracts/generated/casklet";
  import Icon from "./Icon.svelte";
  import { assignCaskColors } from "./caskColors";
  import CaskBadge from "./CaskBadge.svelte";

  let {
    application,
    onClose,
    onAssociated,
    onCreate,
  }: {
    application: DetectedApplicationView;
    onClose: () => void;
    onAssociated: (cask: CaskView) => void;
    onCreate: () => void;
  } = $props();
  let dialog: HTMLDialogElement;
  let loading = $state(true);
  let busy = $state(false);
  let error = $state("");
  let casks: CaskView[] = $state([]);
  let linked: string[] = $state([]);
  let unavailable: string[] = $state([]);
  let query = $state("");
  let destination = $state("");
  const eligible = $derived(
    casks.filter(
      (cask) => !linked.includes(cask.id) && !unavailable.includes(cask.id),
    ),
  );
  const visible = $derived(
    casks.filter((cask) =>
      cask.name.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()),
    ),
  );
  const message = (cause: unknown) =>
    cause && typeof cause === "object" && "message" in cause
      ? String(cause.message)
      : String(cause);
  async function load() {
    loading = true;
    error = "";
    destination = "";
    try {
      casks = (
        await invoke<CaskView[]>("list_casks", {
          request: { version: CONTRACT_VERSION },
        })
      ).filter((cask) => cask.state === "ACTIVE");
      assignCaskColors(casks);
      const results = await Promise.allSettled(
        casks.map((cask) =>
          invoke<DetectedApplicationView[]>("list_associated_applications", {
            request: { version: CONTRACT_VERSION, caskId: cask.id },
          }),
        ),
      );
      linked = [];
      unavailable = [];
      results.forEach((result, index) => {
        if (result.status === "rejected") unavailable.push(casks[index].id);
        else if (
          result.value.some(
            (item) => item.installationId === application.installationId,
          )
        )
          linked.push(casks[index].id);
      });
      if (unavailable.length)
        error =
          "Some Casks could not be checked. Retry to make them available.";
    } catch (cause) {
      casks = [];
      error = message(cause);
    } finally {
      loading = false;
    }
  }
  async function associate() {
    const cask = eligible.find((item) => item.id === destination);
    if (!cask || busy || loading) return;
    busy = true;
    error = "";
    try {
      await invoke("associate_installed_application", {
        request: {
          version: CONTRACT_VERSION,
          caskId: cask.id,
          installationId: application.installationId,
        },
      });
    } catch (cause) {
      error = message(cause);
      busy = false;
      return;
    }
    // Association is committed. Event delivery failure must not invite a duplicate retry.
    void emit("casks-changed").catch(() => {});
    void emit("installed-application-associated", { caskId: cask.id }).catch(
      () => {},
    );
    onAssociated(cask);
  }
  onMount(() => {
    dialog.showModal();
    void load();
  });
</script>

<dialog
  in:surfaceFade
  out:surfaceFade
  bind:this={dialog}
  class="associate-dialog"
  aria-labelledby="associate-title"
  oncancel={(event) => {
    event.preventDefault();
    if (!busy) onClose();
  }}
>
  <form
    onsubmit={(event) => {
      event.preventDefault();
      void associate();
    }}
  >
    <div class="section-heading">
      <h2 id="associate-title">Associate application</h2>
      <button
        class="icon-button"
        type="button"
        aria-label="Close association"
        disabled={busy}
        onclick={onClose}><Icon name="close" /></button
      >
    </div>
    <p class="association-app-name">{application.name}</p>
    <p class="association-explanation">
      Add a reference to a Cask. The application stays installed, and its
      ownership does not change.
    </p>
    {#if error}<div class="association-error">
        <p role="alert">{error}</p>
        <button
          type="button"
          disabled={busy || loading}
          onclick={() => void load()}>Retry Casks</button
        >
      </div>{/if}
    {#if loading}<p role="status" class="association-loading">Loading Casks…</p>
    {:else if !casks.length && !error}<div class="empty-state compact">
        <h3>No active Casks</h3>
        <p>Create a Cask before associating this application.</p>
        <button type="button" class="primary-button" onclick={onCreate}
          >Create a Cask</button
        >
      </div>
    {:else if casks.length}
      <label for="association-search">Search Casks</label><input
        id="association-search"
        type="search"
        bind:value={query}
        oninput={() => {
          destination = "";
        }}
        disabled={busy}
      />
      {#if !eligible.length && !unavailable.length}<p class="association-note">
          This application is already associated with every active Cask.
        </p>
        <button type="button" onclick={onCreate}>Create a Cask</button>{/if}
      <fieldset class="association-destinations" disabled={busy}>
        <legend>Choose a Cask</legend>
        {#each visible as cask (cask.id)}<label
            class:chosen={destination === cask.id}
            class:unavailable={linked.includes(cask.id) ||
              unavailable.includes(cask.id)}
          >
            <input
              type="radio"
              name="association-cask"
              value={cask.id}
              bind:group={destination}
              disabled={linked.includes(cask.id) ||
                unavailable.includes(cask.id)}
            />
            <CaskBadge caskId={cask.id} size={32} />
            <span
              ><strong>{cask.name}</strong>{#if linked.includes(cask.id)}<small
                  >Already associated</small
                >{:else if unavailable.includes(cask.id)}<small
                  >Unable to verify</small
                >{/if}</span
            >
          </label>{:else}<p>No matching Casks. Try another name.</p>{/each}
      </fieldset>
    {/if}
    <div class="dialog-actions">
      <button type="button" disabled={busy} onclick={onClose}>Cancel</button
      ><button
        class="primary-button"
        disabled={busy ||
          loading ||
          !eligible.some((cask) => cask.id === destination)}
        >{busy ? "Associating…" : "Associate"}</button
      >
    </div>
  </form>
</dialog>
