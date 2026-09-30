<script lang="ts">
  import MainWindow from "./MainWindow.svelte";
  import Workspace from "./Workspace.svelte";
  import WindowChrome from "./WindowChrome.svelte";
  import ToastArea from "./ToastArea.svelte";
  import { revealWindow } from "./windowReady";
  import { notify } from "./notifications";
  const ready = () =>
    void revealWindow().catch(() =>
      notify("Unable to reveal the application window.", "error"),
    );
  const caskId = new URLSearchParams(window.location.search).get("cask");
</script>

<WindowChrome />
<div class="window-body">
  {#if caskId}<Workspace {caskId} onReady={ready} />{:else}<MainWindow
      onReady={ready}
    />{/if}
</div>
<ToastArea />
