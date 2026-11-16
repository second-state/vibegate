<script lang="ts">
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import ConnectBar from "./components/ConnectBar.svelte";
  import Canvas from "./components/Canvas.svelte";
  import SessionDrawer from "./components/SessionDrawer.svelte";
  import FileDrawer from "./components/FileDrawer.svelte";
  import Keybar from "./components/Keybar.svelte";
  import { canvasOnly, zoomController, windows, focusPrefix, ANCHOR_X, ANCHOR_Y } from "./lib/canvas";

  // Two-finger TAP anywhere toggles canvas mode: quick (≤350ms) and still
  // (≤12px per finger). A pinch moves — it cancels the tap instead of
  // flipping the mode.
  //
  // In canvas mode the arrow keys focus the nearest window in that direction
  // (falling back to panning when no neighbor exists).
  onMount(() => {
    let down: { t: number; pts: Map<number, number[]> } | null = null;
    const MAX_MS = 350;
    const MAX_MOVE = 12;

    const onPanKey = (e: KeyboardEvent) => {
      if (!get(canvasOnly)) return;
      const dir: "left" | "right" | "up" | "down" | null =
        e.key === "ArrowLeft" ? "left"
        : e.key === "ArrowRight" ? "right"
        : e.key === "ArrowUp" ? "up"
        : e.key === "ArrowDown" ? "down"
        : null;
      if (!dir) return;
      e.preventDefault();
      const wins = get(windows);
      const cur = get(focusPrefix);
      const c = cur ? wins.get(cur) : undefined;
      if (!c) return;
      const ccx = c.x + c.w / 2;
      const ccy = c.y + c.h / 2;
      let bestKey: string | null = null;
      let bestScore = Infinity;
      for (const [key, r] of wins) {
        if (key === cur) continue;
        const rx = r.x + r.w / 2;
        const ry = r.y + r.h / 2;
        const primary =
          dir === "left" ? ccx - (rx + r.w)
          : dir === "right" ? rx - (c.x + c.w)
          : dir === "up" ? ccy - (ry + r.h)
          : ry - (c.y + c.h);
        const perpGap = dir === "left" || dir === "right"
          ? Math.max(0, Math.max(c.y, r.y) - Math.min(c.y + c.h, r.y + r.h))
          : Math.max(0, Math.max(c.x, r.x) - Math.min(c.x + c.w, r.x + r.w));
        if (primary < -40) continue; // mostly behind us — not a neighbor
        const score = Math.max(primary, 0) + perpGap * 2 + (Math.abs(primary) + Math.abs(perpGap)) * 0.1;
        if (score < bestScore) {
          bestScore = score;
          bestKey = key;
        }
      }
      if (bestKey) {
        focusPrefix.set(bestKey);
        // center the newly focused window so focus is always visible
        // (moveTo pos = window center minus the anchor offset, same as
        // double-click-to-center on the canvas)
        const nb = wins.get(bestKey)!;
        const zt = get(zoomController);
        zt?.moveTo(
          [nb.x + nb.w / 2 - ANCHOR_X / zt.zoom, nb.y + nb.h / 2 - ANCHOR_Y / zt.zoom],
          zt.zoom,
        );
        return;
      }
      // no neighbor in that direction: center the focused window
      const zt = get(zoomController);
      zt?.moveTo(
        [ccx - ANCHOR_X / zt.zoom, ccy - ANCHOR_Y / zt.zoom],
        zt.zoom,
      );
    };
    document.addEventListener("keydown", onPanKey, { capture: true });

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
      document.removeEventListener("keydown", onPanKey, { capture: true });
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
