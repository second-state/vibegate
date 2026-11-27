<script lang="ts">
  import { onMount } from "svelte";
  import { TouchZoom, INITIAL_ZOOM } from "../lib/touchZoom";
  import {
    orderedPrefixes,
    hasWindows,
    focusPrefix,
    windows,
    zoomController,
    canvasZoom,
    canvasMoved,
    canvasOnly,
    termCtxRequest,
    ANCHOR_X,
    ANCHOR_Y,
    IMAGE_KEY_PREFIX,
    MD_KEY_PREFIX,
    VIDEO_KEY_PREFIX,
    CAST_KEY_PREFIX,
    blobImages,
  } from "../lib/canvas";
  import { connection } from "../lib/connection";
  import { appearance, resolveBgUrl } from "../lib/appearance";
  import { t } from "../lib/i18n";
  import TermWindow from "./TermWindow.svelte";
  import ImageWindow from "./ImageWindow.svelte";
  import MdWindow from "./MdWindow.svelte";
  import VideoWindow from "./VideoWindow.svelte";
  import CastWindow from "./CastWindow.svelte";

  const connState = connection.state;

  // The window-world anchor: children live at world coords, the anchor sits
  // near the viewport center so world (0,0) appears centerish at zoom 1.
  const anchorLeftCss = `calc(50vw - ${ANCHOR_X}px)`;
  const anchorTopCss = `calc(50vh - ${ANCHOR_Y}px)`;
  const originCss = `calc(-50vw + ${ANCHOR_X}px) calc(-50vh + ${ANCHOR_Y}px)`;

  let fabric: HTMLElement;
  let touchZoom: TouchZoom;
  let center: number[] = [0, 0];
  let zoom = INITIAL_ZOOM;

  /** CSS for the grid backdrop tile: all styles share the 64px world grid,
   *  offset by the pan so tiles stay glued to world coordinates. */
  function gridBackdropStyle(style: "dots" | "lines" | "cross" | "none"): string {
    const s = `${64 * zoom}px ${64 * zoom}px`;
    const pos = `${-zoom * center[0]}px ${-zoom * center[1]}px`;
    if (style === "lines") {
      return (
        `background-image:linear-gradient(to right,#262a3d 1px,transparent 1px),` +
        `linear-gradient(to bottom,#262a3d 1px,transparent 1px);` +
        `background-size:${s};background-position:${pos};`
      );
    }
    if (style === "cross") {
      const tile =
        `url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='64' height='64'>` +
        `<path d='M32 26v12M26 32h12' stroke='%23262a3d' stroke-width='2.5'/></svg>")`;
      return `background-image:${tile};background-size:${s};background-position:${pos};`;
    }
    return `background-image:radial-gradient(#262a3d ${zoom}px,transparent 0);background-size:${s};background-position:${pos};`;
  }

  onMount(() => {
    touchZoom = new TouchZoom(fabric);
    fabric.tabIndex = -1; // focusable: entering canvas mode focuses it
    zoomController.set(touchZoom);
    // Middle-click on empty canvas toggles canvas mode (desktop shortcut).
    // Only when the hit element is the fabric itself AND the point is not
    // inside any window — middle-click over a window (e.g. primary-selection
    // paste into a terminal) is left alone.
    const middleToggle = (e: MouseEvent) => {
      if (e.button !== 1) return;
      if (document.elementFromPoint(e.clientX, e.clientY) !== fabric) return;
      const insideWindow = [...document.querySelectorAll("[data-window]")].some((el) => {
        const r = el.getBoundingClientRect();
        return (
          e.clientX >= r.left &&
          e.clientX <= r.right &&
          e.clientY >= r.top &&
          e.clientY <= r.bottom
        );
      });
      if (insideWindow) return;
      e.preventDefault(); // no autoscroll
      canvasOnly.update((v) => !v);
    };
    fabric.addEventListener("mousedown", middleToggle);
    // Long-press (touch, canvas mode only) on a term pane → open its context
    // menu: there is no right-click on mobile, and in canvas mode the pane is
    // click-through, so the touch lands on the fabric and the canvas layer
    // resolves the window. Headers stay interactive → excluded via
    // [data-window]. Movement (pan) or a second finger (pinch) cancels.
    let lpTimer: ReturnType<typeof setTimeout> | null = null;
    let lpStart: { x: number; y: number; id: number } | null = null;
    const lpCancel = () => {
      if (lpTimer) clearTimeout(lpTimer);
      lpTimer = null;
      lpStart = null;
    };
    const lpDown = (e: PointerEvent) => {
      if (!$canvasOnly || e.pointerType !== "touch") return;
      if (lpStart) {
        lpCancel(); // second finger joins → pinch, not a press
        return;
      }
      if (e.target instanceof Element && e.target.closest("[data-window]")) return;
      lpStart = { x: e.clientX, y: e.clientY, id: e.pointerId };
      lpTimer = setTimeout(() => {
        lpTimer = null;
        const start = lpStart;
        lpStart = null;
        if (!start) return;
        // Topmost term window under the finger (document order = paint order);
        // only term windows carry data-prefix.
        const panes = document.querySelectorAll<HTMLElement>("[data-prefix]");
        for (let i = panes.length - 1; i >= 0; i--) {
          const r = panes[i].getBoundingClientRect();
          if (start.x >= r.left && start.x <= r.right && start.y >= r.top && start.y <= r.bottom) {
            termCtxRequest.set({ prefix: panes[i].dataset.prefix!, x: start.x, y: start.y });
            return;
          }
        }
      }, 500);
    };
    const lpMove = (e: PointerEvent) => {
      if (
        lpStart &&
        e.pointerId === lpStart.id &&
        (Math.abs(e.clientX - lpStart.x) > 10 || Math.abs(e.clientY - lpStart.y) > 10)
      ) {
        lpCancel(); // turned into a pan gesture
      }
    };
    fabric.addEventListener("pointerdown", lpDown);
    fabric.addEventListener("pointermove", lpMove);
    window.addEventListener("pointerup", lpCancel);
    window.addEventListener("pointercancel", lpCancel);
    const off = touchZoom.onMove(() => {
      center = touchZoom.center;
      zoom = touchZoom.zoom;
      canvasZoom.set(touchZoom.zoom);
      canvasMoved.update((n) => n + 1);
      // Blur the focused terminal so panning keeps working when the pointer
      // crosses its (hidden) textarea.
      const active = document.activeElement;
      if (active && active.classList.contains("xterm-helper-textarea")) {
        (active as HTMLElement).blur();
      }
    });
    return () => {
      off();
      lpCancel();
      fabric.removeEventListener("mousedown", middleToggle);
      fabric.removeEventListener("pointerdown", lpDown);
      fabric.removeEventListener("pointermove", lpMove);
      window.removeEventListener("pointerup", lpCancel);
      window.removeEventListener("pointercancel", lpCancel);
      touchZoom.destroy();
      zoomController.set(null);
    };
  });

  /** Double-click empty canvas: zoom 1x centered on the focused window. */
  function fitFocused() {
    const prefix = $focusPrefix;
    const rect = prefix ? $windows.get(prefix) : null;
    if (!rect) return;
    const wx = rect.x + rect.w / 2;
    const wy = rect.y + rect.h / 2;
    // World point that will land at the viewport center at zoom 1:
    // screen = anchor + zoom * (world - center)  →  center = world - anchor/zoom
    touchZoom.moveTo(
      [wx - ANCHOR_X / INITIAL_ZOOM, wy - ANCHOR_Y / INITIAL_ZOOM],
      INITIAL_ZOOM,
    );
  }
</script>

<div
  bind:this={fabric}
  class="absolute inset-0 touch-none overflow-hidden bg-[#16161e]"
  data-canvas-fabric
  role="presentation"
  on:dblclick={fitFocused}
>
  <!-- Canvas background image (behind everything, dimmed by the
       appearance.canvasBgDim slider for contrast). -->
  {#if $appearance.canvasBg}
    <div
      class="pointer-events-none absolute inset-0 bg-cover bg-center"
      style:background-image={`url("${resolveBgUrl($appearance.canvasBg)}")`}
    ></div>
    <div
      class="pointer-events-none absolute inset-0 bg-black"
      style:opacity={Math.min(100, Math.max(0, $appearance.canvasBgDim ?? 55)) / 100}
    ></div>
  {/if}

  <!-- Canvas grid backdrop, moving with the canvas; style picked in
       appearance (dots / lines / cross / hidden). -->
  {#if ($appearance.canvasGrid ?? "dots") !== "none"}
    <div
      class="pointer-events-none absolute inset-0"
      style={gridBackdropStyle($appearance.canvasGrid ?? "dots")}
    ></div>
  {/if}

  <!-- World container: all windows, scaled and translated by the canvas. -->
  <div
    class="absolute"
    style:left={anchorLeftCss}
    style:top={anchorTopCss}
    style:transform-origin={originCss}
    style:transform="scale({zoom}) translate3d({-center[0]}px, {-center[1]}px, 0)"
  >
    {#each $orderedPrefixes as key (key)}
      {#if key.startsWith(IMAGE_KEY_PREFIX)}
        <ImageWindow path={key.slice(IMAGE_KEY_PREFIX.length)} {zoom} />
      {:else if key.startsWith("blob:")}
        <ImageWindow winKey={key} blob={$blobImages.get(key) ?? null} {zoom} />
      {:else if key.startsWith(MD_KEY_PREFIX)}
        <MdWindow path={key.slice(MD_KEY_PREFIX.length)} {zoom} />
      {:else if key.startsWith(VIDEO_KEY_PREFIX)}
        <VideoWindow path={key.slice(VIDEO_KEY_PREFIX.length)} {zoom} />
      {:else if key.startsWith(CAST_KEY_PREFIX)}
        <CastWindow path={key.slice(CAST_KEY_PREFIX.length)} {zoom} />
      {:else}
        <TermWindow prefix={key} {zoom} />
      {/if}
    {/each}
  </div>

  {#if !$hasWindows}
    <div class="pointer-events-none absolute inset-0 flex items-center justify-center">
      <div class="rounded-xl bg-slate-900/70 px-6 py-4 text-center text-sm text-slate-400">
        <p class="mb-1 text-base text-slate-300">{$t("canvasEmpty")}</p>
        <p>
          {#if $connState === "connected"}
            {$t("canvasEmptyConnect")}
          {:else}
            {$t("canvasEmptyOffline")}
          {/if}
        </p>
        <p class="mt-2 text-xs text-slate-500">{$t("canvasHint")}</p>
      </div>
    </div>
  {/if}
</div>
