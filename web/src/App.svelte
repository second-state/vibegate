<script lang="ts">
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import ConnectBar from "./components/ConnectBar.svelte";
  import Canvas from "./components/Canvas.svelte";
  import SessionDrawer from "./components/SessionDrawer.svelte";
  import FileDrawer from "./components/FileDrawer.svelte";
  import Keybar from "./components/Keybar.svelte";
  import {
    canvasOnly,
    zoomController,
    windows,
    focusPrefix,
    ANCHOR_X,
    ANCHOR_Y,
    getSession,
    drawerKbd,
    drawerSel,
    drawerRows,
    drawerOpen,
    openWindow,
    filesKbd,
    filesOpen,
  } from "./lib/canvas";

  // Two-finger TAP anywhere toggles canvas mode: quick (≤350ms) and still
  // (≤12px per finger). A pinch moves — it cancels the tap instead of
  // flipping the mode.
  //
  // In canvas mode the arrow keys focus the nearest window in that direction
  // (falling back to centering the focused window when no neighbor exists).
  onMount(() => {
    let down: { t: number; pts: Map<number, number[]> } | null = null;
    const MAX_MS = 350;
    const MAX_MOVE = 12;

    /** Exiting canvas mode: give browser focus back to the terminal that was
     *  focused before, so typing works immediately. 50ms: a freshly opened
     *  window (drawer Enter) needs a beat to finish mounting. */
    function refocusFocusedTerm() {
      const prefix = get(focusPrefix);
      const s = prefix ? getSession(prefix) : undefined;
      setTimeout(() => s?.term.textarea?.focus(), 50);
    }
    function toggleCanvasMode() {
      canvasOnly.update((v) => !v);
      if (get(canvasOnly)) {
        // entering: focus the canvas fabric itself — it "wakes" to receive
        // keys (arrows pan) and the terminal textarea releases focus
        (document.querySelector("[data-canvas-fabric]") as HTMLElement | null)?.focus();
      } else {
        refocusFocusedTerm();
      }
    }

    const onPanKey = (e: KeyboardEvent) => {
      // Ctrl+B anywhere toggles canvas mode.
      if (
        e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey &&
        (e.key === "b" || e.key === "B")
      ) {
        e.preventDefault(); // don't leak ^B into the focused terminal
        toggleCanvasMode();
        return;
      }
      if (!get(canvasOnly)) return;
      // Enter in canvas mode → back to normal (terminals operable again).
      if (e.key === "Enter" && !get(drawerKbd)) {
        e.preventDefault();
        canvasOnly.set(false);
        refocusFocusedTerm();
        return;
      }
      // e → sessions drawer cycle: closed → open+focused → closed.
      if (e.key === "e" || e.key === "E") {
        e.preventDefault();
        if (get(drawerKbd)) {
          drawerKbd.set(false);
          drawerOpen.set(false);
        } else {
          drawerOpen.set(true);
          drawerKbd.set(true);
          filesKbd.set(false);
          filesOpen.set(false); // drawers are mutually exclusive
          const rows = get(drawerRows);
          if (!get(drawerSel) || !rows.includes(get(drawerSel)!)) {
            drawerSel.set(rows[0] ?? null);
          }
        }
        return;
      }
      // f → files drawer cycle: closed → open+focused → closed.
      if (e.key === "f" || e.key === "F") {
        e.preventDefault();
        if (get(filesKbd)) {
          filesKbd.set(false);
          filesOpen.set(false);
        } else {
          filesOpen.set(true);
          filesKbd.set(true);
          drawerKbd.set(false);
          drawerOpen.set(false); // drawers are mutually exclusive
        }
        return;
      }
      // files drawer keyboard mode: Escape exits focus mode (drawer stays).
      if (e.key === "Escape" && get(filesKbd)) {
        filesKbd.set(false);
        e.preventDefault();
        return;
      }
      // files drawer keyboard mode: its own listener owns the keys
      if (get(filesKbd) && get(filesOpen)) return;
      // drawer keyboard mode: j/k/arrows navigate, Enter opens.
      if (get(drawerKbd) && get(drawerOpen)) {
        const rows = get(drawerRows);
        const idx = rows.indexOf(get(drawerSel) ?? "");
        const move = (d: number) => {
          if (!rows.length) return;
          drawerSel.set(rows[Math.min(rows.length - 1, Math.max(0, (idx === -1 ? 0 : idx) + d))]);
        };
        if (e.key === "ArrowDown" || e.key === "j") {
          e.preventDefault();
          move(1);
          return;
        }
        if (e.key === "ArrowUp" || e.key === "k") {
          e.preventDefault();
          move(-1);
          return;
        }
        if (e.key === "Enter") {
          e.preventDefault();
          const sel = get(drawerSel);
          // attach onto the canvas; session (drawer kbd) mode persists so
          // more sessions can be attached without leaving the drawer
          if (sel) openWindow(sel);
          return;
        }
        if (e.key === "Escape") {
          drawerKbd.set(false);
          e.preventDefault();
          return;
        }
      }
      const dir: "left" | "right" | "up" | "down" | null =
        e.key === "ArrowLeft" || e.key === "h" ? "left"
        : e.key === "ArrowRight" || e.key === "l" ? "right"
        : e.key === "ArrowUp" || e.key === "k" ? "up"
        : e.key === "ArrowDown" || e.key === "j" ? "down"
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
        e.preventDefault(); // suppress the synthetic click on whatever was tapped
        toggleCanvasMode();
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
