<script lang="ts">
  import { onMount } from "svelte";
  import ConnectBar from "./components/ConnectBar.svelte";
  import Canvas from "./components/Canvas.svelte";
  import SessionDrawer from "./components/SessionDrawer.svelte";
  import Keybar from "./components/Keybar.svelte";
  import { discovery } from "./lib/discovery";

  onMount(() => {
    discovery.attach();
  });
</script>

<div class="fixed inset-0 flex flex-col text-slate-200 select-none">
  <ConnectBar />
  <div class="relative flex-1 min-h-0">
    <Canvas />
    <SessionDrawer />
  </div>
  <Keybar />
</div>
