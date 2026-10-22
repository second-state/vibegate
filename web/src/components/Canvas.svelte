<script lang="ts">
  import { onMount } from "svelte";
  import { TouchZoom, INITIAL_ZOOM } from "../lib/touchZoom";
  import {
    orderedPrefixes,
    hasWindows,
    focusPrefix,
    windows,
  } from "../lib/canvas";
  import { connection } from "../lib/connection";
  import TermWindow from "./TermWindow.svelte";

  const connState = connection.state;

  // The window-world anchor: children live at world coords, the anchor sits
  // near the viewport center so world (0,0) appears centerish at zoom 1.
  const ANCHOR_X = 380;
  const ANCHOR_Y = 260;
  const anchorLeftCss = `calc(50vw - ${ANCHOR_X}px)`;
  const anchorTopCss = `calc(50vh - ${ANCHOR_Y}px)`;
  const originCss = `calc(-50vw + ${ANCHOR_X}px) calc(-50vh + ${ANCHOR_Y}px)`;

  let fabric: HTMLElement;
  let touchZoom: TouchZoom;
  let center: number[] = [0, 0];
  let zoom = INITIAL_ZOOM;

  onMount(() => {
    touchZoom = new TouchZoom(fabric);
    const off = touchZoom.onMove(() => {
      center = touchZoom.center;
      zoom = touchZoom.zoom;
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
    {#each $orderedPrefixes as prefix (prefix)}
      <TermWindow {prefix} {zoom} />
    {/each}
  </div>

  {#if !$hasWindows}
    <div class="pointer-events-none absolute inset-0 flex items-center justify-center">
      <div class="rounded-xl bg-slate-900/70 px-6 py-4 text-center text-sm text-slate-400">
        <p class="mb-1 text-base text-slate-300">画布是空的</p>
        <p>
          {#if $connState === "connected"}
            点右上角「会话」打开一个终端窗口
          {:else}
            先在顶部连接 vibegate 网关
          {/if}
        </p>
        <p class="mt-2 text-xs text-slate-500">拖动空白平移 · 双指/滚轮缩放 · 双击窗口居中</p>
      </div>
    </div>
  {/if}
</div>
