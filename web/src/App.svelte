<script lang="ts">
  import { onMount } from "svelte";
  import ConnectBar from "./components/ConnectBar.svelte";
  import Canvas from "./components/Canvas.svelte";
  import SessionDrawer from "./components/SessionDrawer.svelte";
  import FileDrawer from "./components/FileDrawer.svelte";
  import Keybar from "./components/Keybar.svelte";
  import { canvasOnly } from "./lib/canvas";

  // Two-finger TAP anywhere toggles canvas mode: quick (≤350ms) and still
  // (≤12px per finger). A pinch moves — it cancels the tap instead of
  // flipping the mode.
  onMount(() => {
    let down: { t: number; pts: Map<number, number[]> } | null = null;
    const MAX_MS = 350;
    const MAX_MOVE = 12;
    const start = (e: TouchEvent) => {
      if (e.touches.length === 2) {
        // preventDefault keeps the browser from claiming the tap (otherwise
        // it sends touchcancel and we never see the touchend)
        e.preventDefault();
        down = { t: Date.now(), pts: new Map([...e.touches].map((t) => [t.identifier, [t.clientX, t.clientY]])) };
      } else {
        down = null; // one finger, or 3+ — not a two-finger tap
      }
    };
    const move = (e: TouchEvent) => {
      if (!down) return;
      for (const t of e.touches) {
        const p = down.pts.get(t.identifier);
        if (p && Math.hypot(t.clientX - p[0], t.clientY - p[1]) > MAX_MOVE) {
          down = null; // moving: it's a pinch/pan, not a tap
          return;
        }
      }
    };
    const end = (e: TouchEvent) => {
      if (!down) return;
      if (e.touches.length > 0) return; // one finger lifted, the other still down
      const tapped = Date.now() - down.t < MAX_MS;
      down = null;
      if (tapped) {
        canvasOnly.update((v) => !v);
        e.preventDefault(); // suppress the synthetic click on whatever was tapped
      }
    };
    document.addEventListener("touchstart", start, { capture: true, passive: false });
    document.addEventListener("touchmove", move, { capture: true });
    document.addEventListener("touchend", end, { capture: true, passive: false });
    document.addEventListener("touchcancel", () => (down = null), { capture: true });
    return () => {
      document.removeEventListener("touchstart", start, { capture: true });
      document.removeEventListener("touchmove", move, { capture: true });
      document.removeEventListener("touchend", end, { capture: true });
    };
  });
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
