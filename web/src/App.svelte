<script lang="ts">
  import ConnectBar from "./components/ConnectBar.svelte";
  import Canvas from "./components/Canvas.svelte";
  import SessionDrawer from "./components/SessionDrawer.svelte";
  import FileDrawer from "./components/FileDrawer.svelte";
  import Keybar from "./components/Keybar.svelte";
</script>

<div class="fixed inset-0 flex flex-col text-slate-200 select-none">
  <ConnectBar />
  <div class="relative flex-1 min-h-0">
    <Canvas />
    <SessionDrawer />
    <FileDrawer />
    <!-- Keybar floats just below the focused terminal window (fixed, so the
         canvas never scales it). -->
    <Keybar />
  </div>
</div>
