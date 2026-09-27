<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import type { AddMode, ApiError, CaskView, CategoryView, ResourceView, StorageSummary, RecoveryStatus, RecoveryInspection, OperationView, RemovalPlanView, PortableZipPreview, PortableInstallationView, DetectedApplicationView, WingetCapabilities, WingetPackageReview, WingetInstallationView, WingetUninstallPlanView, WingetOperationView, WingetSearchResult } from '../contracts/generated/casklet';
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
  let recovery: RecoveryStatus | null = $state(null);
  let recoveryInspection: RecoveryInspection | null = $state(null);
  let activeOperation = $state('');
  let operations: OperationView[] = $state([]);
  let removalPlan: RemovalPlanView | null = $state(null);
  let removalCategory = $state('');
  let removalEnabled = $state(false);
  let removalConfirmation = $state('');
  let permanentPlan = $state(false);
  let permanentConfirmation = $state(false);
  let removalBusy = $state(false);
  let portablePreview: PortableZipPreview | null = $state(null);
  let portablePath = $state('');
  let portableExe = $state('');
  let portableName = $state('');
  let portableVersion = $state('');
  let portableItems: PortableInstallationView[] = $state([]);
  let portableBusy = $state(false);
  let installedQuery = $state('');
  let detectedApps: DetectedApplicationView[] = $state([]);
  let associatedApps: DetectedApplicationView[] = $state([]);
  let installedBusy = $state(false);
  let wingetCapabilities: WingetCapabilities | null = $state(null);
  let wingetQuery = $state('');
  let wingetSearchText = $state('');
  let wingetPackageId = $state('');
  let wingetVersion = $state('');
  let wingetReview: WingetPackageReview | null = $state(null);
  let wingetAccept = $state(false);
  let wingetItems: WingetInstallationView[] = $state([]);
  let wingetPlan: WingetUninstallPlanView | null = $state(null);
  let wingetOperationId = $state('');
  let wingetOperation: WingetOperationView | null = $state(null);
  let wingetBusy = $state(false);
  let wingetConfirmation = $state('');

  async function searchWinget() {
    wingetBusy=true;
    try {const result=await invoke<WingetSearchResult>('search_winget',{request:{version:CONTRACT_VERSION,query:wingetQuery}});wingetSearchText=result.sanitizedText;error='';}
    catch(cause){error=message(cause);}finally{wingetBusy=false;}
  }
  async function reviewWinget() {
    wingetBusy=true;wingetReview=null;wingetAccept=false;
    try {wingetReview=await invoke<WingetPackageReview>('review_winget_package',{request:{version:CONTRACT_VERSION,packageId:wingetPackageId,packageVersion:wingetVersion}});error='';}
    catch(cause){error=message(cause);}finally{wingetBusy=false;}
  }
  async function installWinget() {
    if(!selected || !wingetReview || !wingetAccept) return;
    wingetBusy=true;
    try {await invoke<WingetInstallationView>('install_winget_package',{request:{version:CONTRACT_VERSION,caskId:selected.id,packageId:wingetReview.packageId,packageVersion:wingetReview.packageVersion,reviewHash:wingetReview.reviewHash,acceptPackageAgreements:true}});wingetReview=null;wingetAccept=false;progress='WinGet installation verified and recorded.';await refreshWinget();error='';}
    catch(cause){error=message(cause);}
    finally{wingetBusy=false;if(wingetOperationId) void refreshWingetOperation();}
  }
  async function refreshWinget(){if(selected)wingetItems=await invoke<WingetInstallationView[]>('list_winget_installations',{request:{version:CONTRACT_VERSION,caskId:selected.id}});}
  async function refreshWingetOperation(){if(wingetOperationId)wingetOperation=await invoke<WingetOperationView>('get_winget_operation',{request:{version:CONTRACT_VERSION,id:wingetOperationId}});}
  async function cancelWinget(){if(!wingetOperationId)return;try{await invoke('cancel_winget_operation',{request:{version:CONTRACT_VERSION,id:wingetOperationId}});await refreshWingetOperation();}catch(cause){error=message(cause);}}
  async function closeWinget(){if(!wingetOperationId)return;try{await invoke('close_winget_operation',{request:{version:CONTRACT_VERSION,id:wingetOperationId}});await refreshWingetOperation();progress='Uncertain WinGet work kept for manual inspection.';}catch(cause){error=message(cause);}}
  async function planWingetUninstall(item:WingetInstallationView){wingetBusy=true;try{wingetPlan=await invoke<WingetUninstallPlanView>('create_winget_uninstall_plan',{request:{version:CONTRACT_VERSION,installationId:item.id}});wingetConfirmation='';error='';}catch(cause){error=message(cause);}finally{wingetBusy=false;}}
  async function approveWingetUninstall(){if(!wingetPlan)return;try{wingetPlan=await invoke<WingetUninstallPlanView>('approve_winget_uninstall_plan',{request:{version:CONTRACT_VERSION,planId:wingetPlan.id,planHash:wingetPlan.planHash}});error='';}catch(cause){error=message(cause);}}
  async function executeWingetUninstall(){if(!wingetPlan || wingetConfirmation!==wingetPlan.id)return;wingetBusy=true;try{wingetPlan=await invoke<WingetUninstallPlanView>('execute_winget_uninstall_plan',{request:{version:CONTRACT_VERSION,planId:wingetPlan.id,planHash:wingetPlan.planHash}});await refreshWinget();progress='WinGet uninstall verified.';error='';}catch(cause){error=message(cause);}finally{wingetBusy=false;if(wingetOperationId)void refreshWingetOperation();}}

  async function searchInstalled() {
    installedBusy = true;
    try { detectedApps = await invoke<DetectedApplicationView[]>('search_installed_applications',{request:{version:CONTRACT_VERSION,query:installedQuery}});error=''; }
    catch (cause) { error=message(cause); }
    finally { installedBusy=false; }
  }
  async function refreshInstalled() {
    installedBusy=true;
    try { const count=await invoke<number>('refresh_installed_applications',{request:{version:CONTRACT_VERSION}});progress=`Reconciled ${count} detected installations.`;await searchInstalled();await refreshAssociated();error=''; }
    catch (cause) { error=message(cause); }
    finally { installedBusy=false; }
  }
  async function refreshAssociated() {
    if (!selected) return;
    associatedApps=await invoke<DetectedApplicationView[]>('list_associated_applications',{request:{version:CONTRACT_VERSION,caskId:selected.id}});
  }
  async function associateDetected(item: DetectedApplicationView) {
    if (!selected) return;
    installedBusy=true;
    try { await invoke('associate_installed_application',{request:{version:CONTRACT_VERSION,caskId:selected.id,installationId:item.installationId}});await refreshAssociated();progress='Associated existing installation. Casklet did not install or own it.';error=''; }
    catch (cause) { error=message(cause); }
    finally { installedBusy=false; }
  }
  async function disassociateDetected(item: DetectedApplicationView) {
    if (!selected) return;
    installedBusy=true;
    try { await invoke('disassociate_installed_application',{request:{version:CONTRACT_VERSION,caskId:selected.id,installationId:item.installationId}});await refreshAssociated();progress='Association removed. The installation is unchanged.';error=''; }
    catch (cause) { error=message(cause); }
    finally { installedBusy=false; }
  }

  async function inspectPortableArchive() {
    try {
      const path = await open({ multiple:false, filters:[{name:'ZIP archive',extensions:['zip']}] });
      if (typeof path !== 'string') return;
      portableBusy = true;
      portablePreview = await invoke<PortableZipPreview>('inspect_portable_zip', {request:{version:CONTRACT_VERSION,archivePath:path}});
      portablePath = path;
      portableExe = portablePreview.executablePaths.length === 1 ? portablePreview.executablePaths[0] : '';
      portableName = path.split(/[\\/]/).pop()?.replace(/\.zip$/i,'') ?? '';
      error = '';
    } catch (cause) {portablePreview=null;error=message(cause);}
    finally {portableBusy=false;}
  }
  async function installPortableArchive() {
    if (!selected || !portablePreview || !portableExe || !portableName.trim()) return;
    portableBusy = true;
    try {
      await invoke<PortableInstallationView>('install_portable_zip',{request:{version:CONTRACT_VERSION,caskId:selected.id,
        archivePath:portablePath,archiveSha256:portablePreview.archiveSha256,applicationName:portableName,
        applicationVersion:portableVersion.trim() || null,executableRelativePath:portableExe}});
      portablePreview=null;portablePath='';portableExe='';portableVersion='';
      progress='Portable application installed. Launch only if you trust its source.';
      await refreshResources();await refreshOperations();error='';
    } catch (cause) {error=message(cause);await refreshRecovery();}
    finally {portableBusy=false;}
  }
  async function launchPortable(item: PortableInstallationView) {
    try { await invoke('launch_portable_installation',{request:{version:CONTRACT_VERSION,installationId:item.installationId}});progress=`Started ${item.applicationName}.`;error=''; }
    catch (cause) {error=message(cause);}
  }
  async function previewPortableRemoval(item: PortableInstallationView) {
    await previewRemoval([item.resourceId]);
  }
  let recoveryManifestPath = $state('');
  let recoveryManifestPreview = $state('');
  let discoveredManifests = $state('');

  async function scanRecoveryManifests() {
    try {
      discoveredManifests = await invoke<string>('scan_recovery_manifests');
      if (!discoveredManifests) discoveredManifests = 'No local manifests found.';
      error = '';
    } catch (cause) { discoveredManifests = ''; error = message(cause); }
  }
  async function recoverDiscoveredManifests() {
    try {
      progress = await invoke<string>('recover_discovered_manifests');
      discoveredManifests = '';
      await refresh();
    } catch (cause) { error = message(cause); }
  }

  async function exportCask(format: 'json' | 'markdown', machine = false) {
    if (!selected) return;
    try {
      const outputPath = await save({ defaultPath: `${selected.name}-inventory.${format === 'json' ? 'json' : 'md'}` });
      if (!outputPath) return;
      await invoke('export_cask', { caskId: selected.id, outputPath, format, machine });
      progress = 'Inventory saved. Review it before sharing.';
      error = '';
    } catch (cause) { error = message(cause); }
  }
  async function chooseRecoveryManifest() {
    try {
      const path = await open({ multiple: false, filters: [{ name: 'Casklet manifest', extensions: ['json'] }] });
      if (typeof path !== 'string') return;
      recoveryManifestPath = path;
      recoveryManifestPreview = await invoke<string>('preview_recovery_manifest', { path });
      error = '';
    } catch (cause) { recoveryManifestPreview = ''; error = message(cause); }
  }
  async function recoverFromManifest() {
    if (!recoveryManifestPath || !recoveryManifestPreview) return;
    try {
      progress = await invoke<string>('recover_manifest', { path: recoveryManifestPath });
      recoveryManifestPath = ''; recoveryManifestPreview = '';
      await refresh();
    } catch (cause) { error = message(cause); }
  }

  async function previewRemoval(resourceIds: string[] = []) {
    if (!selected) return;
    try {
      removalPlan = await invoke<RemovalPlanView>('create_removal_plan', { request: {
        version: CONTRACT_VERSION, caskId: selected.id, resourceIds,
        categoryKey: resourceIds.length ? null : (removalCategory || null),
        permanent: permanentPlan,
      } });
      permanentConfirmation = false;
      error = '';
    } catch (cause) { error = message(cause); }
  }
  function previewSelectedResource() {
    if (selectedResource) void previewRemoval([selectedResource.id]);
  }

  async function approveRemoval() {
    if (!removalPlan || !removalEnabled) return;
    removalBusy = true;
    try {
      removalPlan = await invoke<RemovalPlanView>('approve_removal_plan', { request: {
        version: CONTRACT_VERSION, id: removalPlan.id, planHash: removalPlan.planHash,
      } });
      error = '';
    } catch (cause) { error = message(cause); }
    finally { removalBusy = false; }
  }
  async function executeRemoval() {
    if (!removalPlan || !removalEnabled || removalConfirmation !== removalPlan.id || (removalPlan.permanent && !permanentConfirmation)) return;
    removalBusy = true;
    try {
      removalPlan = await invoke<RemovalPlanView>('execute_removal_plan', { request: {
        version: CONTRACT_VERSION, id: removalPlan.id, planHash: removalPlan.planHash,
        confirmPermanent: removalPlan.permanent && permanentConfirmation,
      } });
      removalConfirmation = '';
      await refreshResources();
      if (selected) selected = await invoke<CaskView>('get_cask', { request: { version: CONTRACT_VERSION, id: selected.id } });
      error = '';
    } catch (cause) { error = message(cause); }
    finally { removalBusy = false; }
  }
  async function closeUncertainRemoval() {
    if (!removalPlan?.runId || removalConfirmation !== removalPlan.runId) return;
    removalBusy = true;
    try {
      removalPlan = await invoke<RemovalPlanView>('close_uncertain_removal', { request: {
        version: CONTRACT_VERSION, runId: removalPlan.runId,
      } });
      removalConfirmation = '';
      if (selected) selected = await invoke<CaskView>('get_cask', { request: { version: CONTRACT_VERSION, id: selected.id } });
      error = '';
    } catch (cause) { error = message(cause); }
    finally { removalBusy = false; }
  }

  function message(cause: unknown): string {
    const api = cause as Partial<ApiError>;
    return api?.message ?? String(cause);
  }
  async function refresh() {
    try {
      casks = await invoke<CaskView[]>('list_casks', { request: { version: CONTRACT_VERSION } });
      await refreshRecovery();
      await refreshOperations();
      error = '';
    } catch (cause) { error = message(cause); }
  }
  async function refreshRecovery() {
    recovery = await invoke<RecoveryStatus>('recovery_status');
  }
  async function refreshOperations() {
    operations = await invoke<OperationView[]>('list_operations', { request: { version: CONTRACT_VERSION, caskId: null } });
  }
  async function recoverOperation(id: string) {
    try {
      await invoke('recover_operation', { request: { version: CONTRACT_VERSION, id } });
      await refreshRecovery();
      await refreshResources();
    } catch (cause) { error = message(cause); await refreshRecovery(); }
  }
  async function retryManifests() {
    try { await invoke('retry_manifests'); await refreshRecovery(); }
    catch (cause) { error = message(cause); }
  }
  async function inspectRecovery(id: string) {
    try { recoveryInspection = await invoke<RecoveryInspection>('inspect_recovery', { request: { version: CONTRACT_VERSION, id } }); }
    catch (cause) { error = message(cause); }
  }
  async function preserveOperation(id: string) {
    try { await invoke('preserve_operation', { request: { version: CONTRACT_VERSION, id } }); await refreshRecovery(); }
    catch (cause) { error = message(cause); }
  }
  async function cancelOperation(id: string) {
    try { await invoke('cancel_operation', { request: { version: CONTRACT_VERSION, id } }); progress = 'Cancellation requested. Casklet will stop at a safe boundary.'; await refreshOperations(); }
    catch (cause) { error = message(cause); }
  }
  async function saveDiagnostics() {
    try {
      const outputPath = await save({ defaultPath: 'casklet-diagnostics.json' });
      if (outputPath) { await invoke('create_diagnostics_bundle', { request: { version: CONTRACT_VERSION, outputPath } }); progress = 'Diagnostic bundle saved.'; }
    } catch (cause) { error = message(cause); }
  }
  async function create() {
    busy = true;
    try {
      selected = await invoke<CaskView>('create_cask', { request: { version: CONTRACT_VERSION, name } });
      pendingPath = ''; pendingMode = ''; selectedResource = null; removalPlan = null;
      name = '';
      await refresh();
      await refreshResources();
      await refreshRecovery();
    } catch (cause) { error = message(cause); }
    finally { busy = false; }
  }
  async function inspect(id: string) {
    try { selected = await invoke<CaskView>('get_cask', { request: { version: CONTRACT_VERSION, id } }); pendingPath = ''; pendingMode = ''; selectedResource = null; removalPlan = null; error = ''; await refreshResources(); }
    catch (cause) { error = message(cause); }
  }
  async function refreshResources() {
    if (!selected) return;
    const caskId = selected.id;
    resources = await invoke<ResourceView[]>('list_resources', { request: { version: CONTRACT_VERSION, caskId } });
    portableItems = await invoke<PortableInstallationView[]>('list_portable_installations', {request:{version:CONTRACT_VERSION,caskId}});
    await refreshWinget().catch(() => {wingetItems=[];});
    await refreshAssociated().catch(() => { associatedApps=[]; });
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
    finally { await refreshRecovery().catch(() => {}); await refreshOperations().catch(() => {}); activeOperation = ''; busy = false; }
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
    void invoke<boolean>('removal_capability').then((enabled) => { removalEnabled = enabled; }).catch(() => { removalEnabled = false; });
    void invoke<WingetCapabilities>('winget_capabilities').then((value)=>{wingetCapabilities=value;}).catch(()=>{wingetCapabilities=null;});
    let disposed = false;
    let unlistenDrop: (() => void) | undefined;
    let unlistenProgress: (() => void) | undefined;
    let unlistenStarted: (() => void) | undefined;
    let unlistenWinget: (() => void) | undefined;
    const poll = window.setInterval(() => { void refreshRecovery().catch(() => {}); void refreshOperations().catch(() => {}); void refreshWingetOperation().catch(()=>{}); }, 3000);
    void getCurrentWindow().onDragDropEvent((event) => {
      if (event.payload.type === 'drop' && event.payload.paths.length > 0) {
        if (event.payload.paths.length > 1) { error = 'Drop one item at a time.'; return; }
        pendingPath = event.payload.paths[0]; pendingMode = '';
      }
    }).then((unlisten) => { if (disposed) unlisten(); else unlistenDrop = unlisten; }).catch((cause) => { error = message(cause); });
    void listen<[number, number]>('resource.progress', (event) => {
      const [done, total] = event.payload; progress = total === 0 ? `${done} entries inspected` : `${done} of ${total} bytes processed`;
    }).then((unlisten) => { if (disposed) unlisten(); else unlistenProgress = unlisten; });
    void listen<string>('operation.started', (event) => { activeOperation = event.payload; })
      .then((unlisten) => { if (disposed) unlisten(); else unlistenStarted = unlisten; });
    void listen<string>('winget.operation.started',(event)=>{wingetOperationId=event.payload;void refreshWingetOperation();}).then((unlisten)=>{if(disposed)unlisten();else unlistenWinget=unlisten;});
    return () => { disposed = true; window.clearInterval(poll); unlistenDrop?.(); unlistenProgress?.(); unlistenStarted?.();unlistenWinget?.(); };
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
  <button type="button" onclick={() => void saveDiagnostics()}>Save diagnostics</button>
  <button type="button" onclick={() => void chooseRecoveryManifest()}>Preview recovery manifest</button>
  <button type="button" onclick={() => void scanRecoveryManifests()}>Scan local recovery manifests</button>
  {#if discoveredManifests}
    <pre>{discoveredManifests}</pre>
    {#if discoveredManifests !== 'No local manifests found.'}<button type="button" onclick={() => void recoverDiscoveredManifests()}>Recover missing Casks as unverified metadata</button>{/if}
  {/if}
  {#if recoveryManifestPreview}
    <p>{recoveryManifestPreview}</p>
    <button type="button" onclick={() => void recoverFromManifest()}>Recover unverified metadata</button>
  {/if}
  {#if operations.some((item) => item.state === 'RUNNING')}
    <section aria-label="Running operations"><h2>Running operations</h2><ul>
      {#each operations.filter((item) => item.state === 'RUNNING') as operation (operation.id)}
        <li>{operation.id}: {operation.step} {operation.progressCompleted}{operation.progressTotal === null ? '' : `/${operation.progressTotal}`}
          <button type="button" disabled={operation.cancellationRequested} onclick={() => void cancelOperation(operation.id)}>{operation.cancellationRequested ? 'Cancellation requested' : 'Cancel'}</button>
        </li>
      {/each}
    </ul></section>
  {/if}
  {#if recovery && (recovery.operations.length || recovery.pendingManifestCasks.length)}
    <section aria-label="Recovery status">
      <h2>Recovery needed</h2>
      <p>Casklet keeps uncertain files for inspection. Recovery only completes work that was already registered and still verifies.</p>
      <ul>{#each recovery.operations as operation (operation.id)}
        <li>Operation {operation.id}: {operation.state.toLowerCase()}
          <button type="button" onclick={() => void inspectRecovery(operation.id)}>Inspect paths</button>
          <button type="button" onclick={() => void recoverOperation(operation.id)}>Verify and recover</button>
          <button type="button" onclick={() => void preserveOperation(operation.id)}>Keep files and close</button>
        </li>
      {/each}</ul>
      {#if recoveryInspection}<dl>
        <dt>Mode</dt><dd>{recoveryInspection.mode}</dd>
        <dt>Source</dt><dd>{recoveryInspection.sourcePath} ({recoveryInspection.sourceStatus})</dd>
        <dt>Staging</dt><dd>{recoveryInspection.stagingPath} ({recoveryInspection.stagingStatus})</dd>
        <dt>Managed destination</dt><dd>{recoveryInspection.finalPath} ({recoveryInspection.finalStatus})</dd>
      </dl>{/if}
      {#if recovery.pendingManifestCasks.length}<p>{recovery.pendingManifestCasks.length} Cask manifests await regeneration. <button type="button" onclick={() => void retryManifests()}>Retry manifests</button></p>{/if}
    </section>
  {/if}
  {#if selected}
    <section aria-label="Cask details">
      <h2>{selected.name}</h2>
      <button type="button" onclick={() => void exportCask('json')}>Export portable JSON</button>
      <button type="button" onclick={() => void exportCask('markdown')}>Export report</button>
      <button type="button" onclick={() => void exportCask('json', true)}>Export machine JSON</button>
      <dl><dt>ID</dt><dd>{selected.id}</dd><dt>State</dt><dd>{selected.state}</dd><dt>Created</dt><dd>{new Date(selected.createdAtMs).toLocaleString()}</dd><dt>Managed storage</dt><dd>{selected.managedStorageRoot}</dd></dl>
      <section aria-label="Portable applications">
        <h3>Portable applications</h3>
        <p>Review a ZIP before installing it. Portable applications are not sandboxed; launch only software you trust.</p>
        <button type="button" disabled={portableBusy} onclick={() => void inspectPortableArchive()}>Inspect portable ZIP</button>
        {#if portablePreview}
          <div class="review">
            <p><strong>Archive:</strong> {portablePath}</p>
            <p><strong>SHA-256:</strong> {portablePreview.archiveSha256}</p>
            <p>{portablePreview.fileCount} files, {portablePreview.expandedBytes} expanded bytes</p>
            <label for="portable-name">Application name</label><input id="portable-name" bind:value={portableName} maxlength="120" />
            <label for="portable-version">Version (optional)</label><input id="portable-version" bind:value={portableVersion} maxlength="80" />
            <label for="portable-executable">Executable to launch</label>
            <select id="portable-executable" bind:value={portableExe}>
              <option value="">Choose executable</option>
              {#each portablePreview.executablePaths as executable (executable)}<option value={executable}>{executable}</option>{/each}
            </select>
            <button type="button" disabled={portableBusy || !portableExe || !portableName.trim()} onclick={() => void installPortableArchive()}>Install into this Cask</button>
          </div>
        {/if}
        {#if portableItems.length}
          <ul>{#each portableItems as item (item.installationId)}
            <li>{item.applicationName} {item.applicationVersion ?? ''} ({item.state}) — {item.executableRelativePath}
              <button type="button" disabled={!item.canLaunch} onclick={() => void launchPortable(item)}>Launch</button>
              <button type="button" disabled={!item.canPlanRemoval} onclick={() => void previewPortableRemoval(item)}>Preview removal</button>
            </li>
          {/each}</ul>
        {/if}
      </section>
      <section aria-label="WinGet packages">
        <h3>WinGet packages</h3>
        <p>WinGet may install for your user or the whole machine. Casklet records ownership only when the package and one new installed instance both verify.</p>
        {#if wingetCapabilities?.search}
          <label for="winget-search">Search WinGet</label><input id="winget-search" bind:value={wingetQuery} maxlength="120" />
          <button type="button" disabled={wingetBusy || !wingetQuery.trim()} onclick={() => void searchWinget()}>Search WinGet</button>
          {#if wingetSearchText}<pre>{wingetSearchText}</pre>{/if}
          <label for="winget-id">Exact package ID</label><input id="winget-id" bind:value={wingetPackageId} maxlength="120" />
          <label for="winget-version">Exact version</label><input id="winget-version" bind:value={wingetVersion} maxlength="80" />
          <button type="button" disabled={wingetBusy || !wingetPackageId || !wingetVersion} onclick={() => void reviewWinget()}>Review package</button>
          {#if wingetReview}
            <div class="review"><p>{wingetReview.packageId} {wingetReview.packageVersion} from {wingetReview.source}</p>
              <pre>{wingetReview.details}</pre>
              <p>{wingetReview.warnings.join(' ')}</p>
              <label><input type="checkbox" bind:checked={wingetAccept} /> I reviewed this package and accept its package agreements</label>
              <button type="button" disabled={wingetBusy || !wingetAccept} onclick={() => void installWinget()}>Install for this Cask</button>
            </div>
          {/if}
        {:else}<p>{wingetCapabilities?.reason ?? 'WinGet capability is being checked.'}</p>{/if}
        {#if wingetOperation}
          <p>Operation {wingetOperation.id}: {wingetOperation.step} ({wingetOperation.state}){wingetOperation.errorCode ? ` — ${wingetOperation.errorCode}` : ''}</p>
          {#if wingetOperation.state==='RUNNING'}<button type="button" disabled={wingetOperation.cancellationRequested} onclick={() => void cancelWinget()}>Cancel WinGet operation</button>{/if}
          {#if wingetOperation.state==='RECOVERY_REQUIRED'}<button type="button" onclick={() => void closeWinget()}>Keep outcome uncertain and close</button>{/if}
        {/if}
        {#if wingetItems.length}<h4>WinGet installations for this Cask</h4><ul>{#each wingetItems as item (item.id)}
          <li>{item.packageId} {item.packageVersion} ({item.state}) — {item.registryInstallationId}
            <p>{item.warnings.join(' ')}</p>
            <button type="button" disabled={wingetBusy || !item.canPlanUninstall} onclick={() => void planWingetUninstall(item)}>Preview uninstall</button>
          </li>
        {/each}</ul>{/if}
        {#if wingetPlan}
          <div class="review"><p>Uninstall plan {wingetPlan.id}: {wingetPlan.packageId} {wingetPlan.packageVersion}, {wingetPlan.state}. Expires {new Date(wingetPlan.expiresAtMs).toLocaleString()}.</p>
            <p>{wingetPlan.warnings.join(' ')}</p>
            {#if wingetPlan.state==='DRAFT'}<button type="button" onclick={() => void approveWingetUninstall()}>Approve exact plan</button>{/if}
            {#if wingetPlan.state==='APPROVED' && wingetCapabilities?.uninstall}
              <label for="winget-confirm">Enter plan ID to confirm uninstall</label><input id="winget-confirm" bind:value={wingetConfirmation} />
              <button type="button" disabled={wingetBusy || wingetConfirmation!==wingetPlan.id} onclick={() => void executeWingetUninstall()}>Execute approved uninstall</button>
            {/if}
            {#if wingetPlan.state==='APPROVED' && !wingetCapabilities?.uninstall}<p>Uninstall execution is pending independent removal review.</p>{/if}
          </div>
        {/if}
      </section>
      <section aria-label="Existing Windows applications">
        <h3>Existing Windows applications</h3>
        <p>Associations are references. Casklet did not install or own these applications and will not uninstall them.</p>
        <label for="installed-search">Search installed applications</label>
        <input id="installed-search" bind:value={installedQuery} maxlength="120" />
        <button type="button" disabled={installedBusy} onclick={() => void searchInstalled()}>Search</button>
        <button type="button" disabled={installedBusy} onclick={() => void refreshInstalled()}>Refresh inventory</button>
        {#if detectedApps.length}
          <ul>{#each detectedApps as item (item.installationId)}
            <li>{item.name} {item.displayVersion ?? ''} — {item.publisher ?? 'Unknown publisher'} ({item.scope})
              {#if item.ambiguousName}<span class="warning">Same display name appears on multiple registrations. Check the registry source.</span>{/if}
              <small>{item.sourceKey}</small>
              <button type="button" disabled={installedBusy || associatedApps.some((a) => a.installationId === item.installationId)} onclick={() => void associateDetected(item)}>Associate with this Cask</button>
            </li>
          {/each}</ul>
        {/if}
        {#if associatedApps.length}
          <h4>Associated installations</h4>
          <ul>{#each associatedApps as item (item.installationId)}
            <li>{item.name} ({item.state}) — {item.sourceKey}
              <button type="button" disabled={installedBusy} onclick={() => void disassociateDetected(item)}>Remove association</button>
            </li>
          {/each}</ul>
        {/if}
      </section>
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
        {#if activeOperation}<button type="button" onclick={() => void cancelOperation(activeOperation)}>Cancel operation</button>{/if}
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
      <section aria-label="Removal preview">
        <h3>Cleanup preview</h3>
        <p>Review what Casklet could detach, preserve, move to the Recycle Bin, or permanently delete. This preview does not change files or associations.</p>
        <label for="removal-category">Scope</label>
        <select id="removal-category" bind:value={removalCategory}>
          <option value="">Whole Cask</option>
          {#each categories as category (category.key)}<option value={category.key}>{category.name}</option>{/each}
        </select>
        <label><input type="checkbox" bind:checked={permanentPlan} /> Permanently delete eligible managed files in this plan</label>
        <p>Permanent deletion cannot be undone. Managed directories remain preserved in a permanent plan; use a Recycle Bin plan for them.</p>
        <button type="button" onclick={() => void previewRemoval()}>Create cleanup preview</button>
        {#if selectedResource}<button type="button" onclick={previewSelectedResource}>Preview selected resource</button>{/if}
        {#if removalPlan}
          <p>Plan {removalPlan.id} is {removalPlan.state.toLowerCase()} and expires {new Date(removalPlan.expiresAtMs).toLocaleString()}. Disposal: {removalPlan.permanent ? 'permanent files' : 'Recycle Bin'}.</p>
          <ul>{#each removalPlan.items as item (item.id)}<li>{item.name}: {item.action.toLowerCase()} ({item.reasonCode.toLowerCase()}); {item.state.toLowerCase()}{item.resultCode ? ` (${item.resultCode.toLowerCase()})` : ''}{#if item.recoveryPath}<br />Possible recovery location: <code>{item.recoveryPath}</code>{/if}</li>{/each}</ul>
          {#if removalEnabled && removalPlan.state === 'DRAFT'}
            <button type="button" disabled={removalBusy} onclick={() => void approveRemoval()}>Approve this plan</button>
          {/if}
          {#if removalEnabled && removalPlan.state === 'APPROVED'}
            <p role="alert">Executing may {removalPlan.permanent ? 'permanently delete eligible managed files' : 'move eligible managed files and directories to the Recycle Bin'} and detach associations. Changed or uncertain items are preserved for review.</p>
            {#if removalPlan.permanent}<label><input type="checkbox" bind:checked={permanentConfirmation} /> I understand that permanent deletion cannot be undone</label>{/if}
            <label for="removal-confirmation">Type the plan ID to execute</label>
            <input id="removal-confirmation" bind:value={removalConfirmation} autocomplete="off" />
            <button type="button" disabled={removalBusy || removalConfirmation !== removalPlan.id || (removalPlan.permanent && !permanentConfirmation)} onclick={() => void executeRemoval()}>Execute cleanup</button>
          {/if}
          {#if removalEnabled && removalPlan.state === 'PARTIAL' && removalPlan.runId}
            <p role="alert">The cleanup outcome is uncertain. Inspect the original path, Recycle Bin, and any recovery location before closing. Closing releases the lock and keeps unresolved outcomes marked unknown.</p>
            <label for="removal-confirmation">Type the run ID to close with preservation</label>
            <input id="removal-confirmation" bind:value={removalConfirmation} autocomplete="off" />
            <button type="button" disabled={removalBusy || removalConfirmation !== removalPlan.runId} onclick={() => void closeUncertainRemoval()}>Close uncertain run</button>
          {/if}
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
