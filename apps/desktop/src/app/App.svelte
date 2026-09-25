<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import type { ApiError, CaskView } from '../contracts/generated/casklet';
  import { CONTRACT_VERSION } from '../contracts/generated/casklet';

  let casks: CaskView[] = $state([]);
  let selected: CaskView | null = $state(null);
  let name = $state('');
  let error = $state('');
  let busy = $state(false);

  function message(cause: unknown): string {
    const api = cause as Partial<ApiError>;
    return api?.message ?? String(cause);
  }
  async function refresh() {
    try {
      casks = await invoke<CaskView[]>('list_casks', { request: { version: CONTRACT_VERSION } });
      error = '';
    } catch (cause) { error = message(cause); }
  }
  async function create() {
    busy = true;
    try {
      selected = await invoke<CaskView>('create_cask', { request: { version: CONTRACT_VERSION, name } });
      name = '';
      await refresh();
    } catch (cause) { error = message(cause); }
    finally { busy = false; }
  }
  async function inspect(id: string) {
    try { selected = await invoke<CaskView>('get_cask', { request: { version: CONTRACT_VERSION, id } }); error = ''; }
    catch (cause) { error = message(cause); }
  }
  onMount(refresh);
</script>

<main>
  <p class="eyebrow">Casklet</p>
  <h1>Your Casks</h1>
  <form onsubmit={(event) => { event.preventDefault(); void create(); }}>
    <label for="cask-name">New Cask name</label>
    <div class="form-row"><input id="cask-name" bind:value={name} maxlength="120" required /><button disabled={busy}>Create Cask</button></div>
  </form>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <section aria-label="Casks">
    {#if casks.length === 0}<p>No Casks yet.</p>{/if}
    <ul>{#each casks as cask (cask.id)}<li><button class="cask-link" onclick={() => void inspect(cask.id)}>{cask.name}</button></li>{/each}</ul>
  </section>
  {#if selected}
    <section aria-label="Cask details">
      <h2>{selected.name}</h2>
      <dl><dt>ID</dt><dd>{selected.id}</dd><dt>State</dt><dd>{selected.state}</dd><dt>Created</dt><dd>{new Date(selected.createdAtMs).toLocaleString()}</dd><dt>Managed storage</dt><dd>{selected.managedStorageRoot}</dd></dl>
      <p>This Cask is empty.</p>
    </section>
  {/if}
</main>
