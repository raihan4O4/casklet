<script lang="ts">
  import MainWindow from "./MainWindow.svelte";
  import Workspace from "./Workspace.svelte";
  import WindowChrome from "./WindowChrome.svelte";
  const caskId = new URLSearchParams(window.location.search).get("cask");
</script>

<WindowChrome />
<div class="window-body">
  {#if caskId}<Workspace {caskId} />{:else}<MainWindow />{/if}
</div>
