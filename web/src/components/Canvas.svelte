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
    ANCHOR_X,
    ANCHOR_Y,
    IMAGE_KEY_PREFIX,
    MD_KEY_PREFIX,
  } from "../lib/canvas";
  import { connection } from "../lib/connection";
  import { appearance, resolveBgUrl } from "../lib/appearance";
  import { t } from "../lib/i18n";
  import TermWindow from "./TermWindow.svelte";
  import ImageWindow from "./ImageWindow.svelte";
  import MdWindow from "./MdWindow.svelte";

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

  onMount(() => {
    touchZoom = new TouchZoom(fabric);
    zoomController.set(touchZoom);
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
  role="presentation"
  on:dblclick={fitFocused}
>
  <!-- Canvas background image (behind everything, dimmed for contrast). -->
  {#if $appearance.canvasBg}
    <div
      class="pointer-events-none absolute inset-0 bg-cover bg-center"
      style:background-image={`url("${resolveBgUrl($appearance.canvasBg)}")`}
    ></div>
    <div class="pointer-events-none absolute inset-0 bg-black/55"></div>
  {/if}

  <!-- Dot grid background, moving with the canvas. -->
  <div
    class="pointer-events-none absolute inset-0"
    style:background-image="radial-gradient(#262a3d {zoom}px, transparent 0)"
    style:background-size="{24 * zoom}px {24 * zoom}px"
    style:background-position="{-zoom * center[0]}px {-zoom * center[1]}px"
  ></div>

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
      {:else if key.startsWith(MD_KEY_PREFIX)}
        <MdWindow path={key.slice(MD_KEY_PREFIX.length)} {zoom} />
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
