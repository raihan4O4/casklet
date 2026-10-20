<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { CaskView } from "../contracts/generated/casklet";
  import { CONTRACT_VERSION } from "../contracts/generated/casklet";
  import Icon from "./Icon.svelte";
  let {
    onCreated,
    onCancel,
    busy = $bindable(false),
    headingId = "create-heading",
    inputId = "cask-name",
    description = "Give this activity a name. Files and applications can be added next.",
  }: {
    onCreated: (cask: CaskView) => Promise<void>;
    onCancel: () => void;
    busy?: boolean;
    headingId?: string;
    inputId?: string;
    description?: string;
  } = $props();
  let name = $state("");
  let error = $state("");
  async function create() {
    if (busy || !name.trim()) return;
    busy = true;
    error = "";
    try {
      const cask = await invoke<CaskView>("create_cask", {
        request: { version: CONTRACT_VERSION, name },
      });
      await onCreated(cask);
    } catch (cause) {
      error =
        typeof cause === "object" && cause !== null && "message" in cause
          ? String(cause.message)
          : String(cause);
    } finally {
      busy = false;
    }
  }
</script>

<form
  onsubmit={(event) => {
    event.preventDefault();
    void create();
  }}
>
  <div class="section-heading">
    <h2 id={headingId}>New Cask</h2>
    <button
      class="icon-button"
      type="button"
      aria-label="Cancel creation"
      disabled={busy}
      onclick={onCancel}><Icon name="close" /></button
    >
  </div>
  <p>{description}</p>
  <label for={inputId}>Cask name</label>
  <input
    id={inputId}
    bind:value={name}
    maxlength="120"
    required
    disabled={busy}
    placeholder="e.g. Photography Course"
  />
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <div class="dialog-actions">
    <button type="button" disabled={busy} onclick={onCancel}>Cancel</button>
    <button class="primary-button" disabled={busy || !name.trim()}
      >{busy ? "Creating…" : "Create Cask"}</button
    >
  </div>
</form>
