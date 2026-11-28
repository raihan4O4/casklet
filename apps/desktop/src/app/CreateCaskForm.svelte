<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { CaskView } from "../contracts/generated/casklet";
  import { CONTRACT_VERSION } from "../contracts/generated/casklet";
  import Icon from "./Icon.svelte";
  import CaskAppearancePicker from "./CaskAppearancePicker.svelte";
  import { DEFAULT_APPEARANCE, saveCaskAppearance } from "./caskAppearance";
  import { assignCaskColors } from "./caskColors";
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
  let appearance = $state({ ...DEFAULT_APPEARANCE });
  async function create() {
    if (busy || !name.trim()) return;
    busy = true;
    error = "";
    try {
      const cask = await invoke<CaskView>("create_cask", {
        request: { version: CONTRACT_VERSION, name },
      });
      assignCaskColors([cask]);
      if (
        appearance.symbol !== DEFAULT_APPEARANCE.symbol ||
        appearance.color !== null
      )
        saveCaskAppearance(cask.id, appearance);
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
  class="create-cask-form"
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
  <div class="create-appearance">
    <span class="appearance-field-label"
      >Appearance <span class="optional-label">Optional</span></span
    ><CaskAppearancePicker bind:value={appearance} disabled={busy} />
  </div>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <div class="dialog-actions">
    <button type="button" disabled={busy} onclick={onCancel}>Cancel</button>
    <button class="primary-button" disabled={busy || !name.trim()}
      >{busy ? "Creating…" : "Create Cask"}</button
    >
  </div>
</form>
