<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import type { AddMode, ApiError, CaskView, CategoryView, ResourceView, StorageSummary } from '../contracts/generated/casklet';
  import { CONTRACT_VERSION } from '../contracts/generated/casklet';

  let casks: CaskView[] = $state([]);
  let selected: CaskView | null = $state(null);
  let name = $state('');
  let error = $state('');
  let busy = $state(false);
  let resources: ResourceView[] = $state([]);
  let categories: CategoryView[] = $state([]);
  let summary: StorageSummary | null = $state(null);
  let pendingPath = $state('');
  let pendingMode: AddMode | '' = $state('');
  let pendingCategory = $state('other');
  let progress = $state('');
  let categoryName = $state('');
  let selectedResource: ResourceView | null = $state(null);

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
      pendingPath = ''; pendingMode = ''; selectedResource = null;
      name = '';
      await refresh();
      await refreshResources();
    } catch (cause) { error = message(cause); }
    finally { busy = false; }
  }
  async function inspect(id: string) {
    try { selected = await invoke<CaskView>('get_cask', { request: { version: CONTRACT_VERSION, id } }); pendingPath = ''; pendingMode = ''; selectedResource = null; error = ''; await refreshResources(); }
    catch (cause) { error = message(cause); }
  }
  async function refreshResources() {
    if (!selected) return;
    const caskId = selected.id;
    resources = await invoke<ResourceView[]>('list_resources', { request: { version: CONTRACT_VERSION, caskId } });
    categories = await invoke<CategoryView[]>('list_categories', { request: { version: CONTRACT_VERSION, caskId } });
    summary = await invoke<StorageSummary>('storage_summary', { request: { version: CONTRACT_VERSION, caskId } });
  }
  async function pick(directory: boolean) {
    try {
      const picked = await open({ multiple: false, directory });
      if (typeof picked === 'string') { pendingPath = picked; pendingMode = ''; }
    } catch (cause) { error = message(cause); }
  }
  async function addResource() {
    if (!selected || !pendingPath || !pendingMode) return;
    busy = true;
    error = '';
    try {
      selectedResource = await invoke<ResourceView>('add_resource', { request: {
        version: CONTRACT_VERSION, caskId: selected.id, sourcePath: pendingPath,
        mode: pendingMode, categoryKey: pendingCategory,
      } });
      pendingPath = ''; pendingMode = ''; progress = '';
      await refreshResources();
    } catch (cause) { error = message(cause); }
    finally { busy = false; }
  }
  async function createCategory() {
    if (!selected || !categoryName.trim()) return;
    try {
      const category = await invoke<CategoryView>('create_category', { request: { version: CONTRACT_VERSION, caskId: selected.id, name: categoryName } });
      categories = [...categories, category]; categoryName = ''; pendingCategory = category.key;
    } catch (cause) { error = message(cause); }
  }
  async function recategorize(resource: ResourceView, categoryKey: string) {
    try {
      selectedResource = await invoke<ResourceView>('categorize_resource', { request: { version: CONTRACT_VERSION, resourceId: resource.id, categoryKey } });
      await refreshResources();
    } catch (cause) { error = message(cause); }
  }
  onMount(() => {
    void refresh();
    let disposed = false;
    let unlistenDrop: (() => void) | undefined;
    let unlistenProgress: (() => void) | undefined;
    void getCurrentWindow().onDragDropEvent((event) => {
      if (event.payload.type === 'drop' && event.payload.paths.length > 0) {
        if (event.payload.paths.length > 1) { error = 'Drop one item at a time.'; return; }
        pendingPath = event.payload.paths[0]; pendingMode = '';
      }
    }).then((unlisten) => { if (disposed) unlisten(); else unlistenDrop = unlisten; }).catch((cause) => { error = message(cause); });
    void listen<[number, number]>('resource.progress', (event) => {
      const [done, total] = event.payload; progress = total === 0 ? `${done} entries inspected` : `${done} of ${total} bytes processed`;
    }).then((unlisten) => { if (disposed) unlisten(); else unlistenProgress = unlisten; });
    return () => { disposed = true; unlistenDrop?.(); unlistenProgress?.(); };
  });
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
      <section aria-label="Resource intake">
        <h3>Add a resource</h3>
        <p>Choose a file or folder, or drop one into this window. Review the action before adding it.</p>
        <div class="form-row"><button type="button" onclick={() => void pick(false)}>Choose file</button><button type="button" onclick={() => void pick(true)}>Choose folder</button></div>
        {#if pendingPath}
          <div class="review">
            <p><strong>Source:</strong> {pendingPath}</p>
            <fieldset><legend>How should Casklet add it?</legend>
              <label><input type="radio" name="intake-mode" value="copy" bind:group={pendingMode} /> Copy into managed storage</label>
              <label><input type="radio" name="intake-mode" value="move" bind:group={pendingMode} /> Move into managed storage</label>
              <label><input type="radio" name="intake-mode" value="reference" bind:group={pendingMode} /> Reference at current location</label>
            </fieldset>
            {#if pendingMode === 'move'}<p class="warning">Move changes the source location. Casklet will verify the managed destination before recording it.</p>{/if}
            <label for="intake-category">Category</label>
            <select id="intake-category" bind:value={pendingCategory}>{#each categories as category (category.key)}<option value={category.key}>{category.name}</option>{/each}</select>
            <button type="button" disabled={busy || !pendingMode} onclick={() => void addResource()}>Add resource</button>
          </div>
        {/if}
        {#if progress}<p role="status">{progress}</p>{/if}
      </section>
      <section aria-label="Resource inventory">
        <h3>Resources</h3>
        {#if resources.length === 0}<p>No resources yet.</p>{/if}
        <ul>{#each resources as resource (resource.id)}
          <li><button class="cask-link" onclick={() => { selectedResource = resource; }}>{resource.name}</button>
            <span> {resource.management.toLowerCase()}</span>
            <label>Category <select value={resource.categoryKey} onchange={(event) => void recategorize(resource, event.currentTarget.value)}>
              {#each categories as category (category.key)}<option value={category.key}>{category.name}</option>{/each}
            </select></label>
          </li>
        {/each}</ul>
        {#if selectedResource}
          <dl><dt>Name</dt><dd>{selectedResource.name}</dd><dt>Location</dt><dd>{selectedResource.path}</dd>
            <dt>Management</dt><dd>{selectedResource.management}</dd><dt>Availability</dt><dd>{selectedResource.availability}</dd>
            <dt>Size</dt><dd>{selectedResource.sizeBytes ?? 'Unknown'} ({selectedResource.sizeMeasurement})</dd></dl>
        {/if}
      </section>
      <section aria-label="Categories">
        <h3>New category</h3>
        <form onsubmit={(event) => { event.preventDefault(); void createCategory(); }}>
          <label for="category-name">Name</label><div class="form-row"><input id="category-name" bind:value={categoryName} maxlength="80" /><button>Create category</button></div>
        </form>
      </section>
      {#if summary}<section aria-label="Storage summary"><h3>Storage</h3><dl>
        <dt>Managed logical</dt><dd>{summary.managedLogicalBytes} bytes</dd>
        <dt>Referenced logical</dt><dd>{summary.referencedLogicalBytes} bytes</dd>
        <dt>Physical</dt><dd>{summary.physicalBytes === null ? 'Unknown' : `${summary.physicalBytes} bytes`}</dd>
        <dt>Reclaimable range</dt><dd>{summary.reclaimableMinBytes}–{summary.reclaimableMaxBytes} bytes</dd>
      </dl></section>{/if}
    </section>
  {/if}
</main>
