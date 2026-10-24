<script lang="ts">
  import Boxes from "@lucide/svelte/icons/boxes";
  import Grid2X2 from "@lucide/svelte/icons/grid-2x2";
  import Activity from "@lucide/svelte/icons/activity";
  import Settings2 from "@lucide/svelte/icons/settings-2";
  import Search from "@lucide/svelte/icons/search";
  import Plus from "@lucide/svelte/icons/plus";
  import HardDrive from "@lucide/svelte/icons/hard-drive";
  import File from "@lucide/svelte/icons/file";
  import Image from "@lucide/svelte/icons/image";
  import Film from "@lucide/svelte/icons/film";
  import FileText from "@lucide/svelte/icons/file-text";
  import Music2 from "@lucide/svelte/icons/music-2";
  import Folder from "@lucide/svelte/icons/folder";
  import MoreHorizontal from "@lucide/svelte/icons/ellipsis";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import ChevronUp from "@lucide/svelte/icons/chevron-up";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import X from "@lucide/svelte/icons/x";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Package from "@lucide/svelte/icons/package";
  import Link2 from "@lucide/svelte/icons/link-2";
  import Clock from "@lucide/svelte/icons/clock";
  import Download from "@lucide/svelte/icons/download";
  import Archive from "@lucide/svelte/icons/archive";
  import Database from "@lucide/svelte/icons/database";
  import Check from "@lucide/svelte/icons/check";
  import Info from "@lucide/svelte/icons/info";
  import List from "@lucide/svelte/icons/list";
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  const icons = {
    casks: Boxes,
    installations: Grid2X2,
    activity: Activity,
    settings: Settings2,
    search: Search,
    plus: Plus,
    storage: HardDrive,
    file: File,
    image: Image,
    video: Film,
    document: FileText,
    audio: Music2,
    folder: Folder,
    more: MoreHorizontal,
    back: ArrowLeft,
    trash: Trash2,
    down: ChevronDown,
    up: ChevronUp,
    open: ExternalLink,
    close: X,
    refresh: RefreshCw,
    package: Package,
    link: Link2,
    clock: Clock,
    download: Download,
    archive: Archive,
    database: Database,
    check: Check,
    info: Info,
    list: List,
    forward: ArrowRight,
  };
  let { name, size = 18 }: { name: keyof typeof icons; size?: number } =
    $props();
  const Component = $derived(icons[name]);
</script>

<Component {size} strokeWidth={1.75} aria-hidden="true" />
