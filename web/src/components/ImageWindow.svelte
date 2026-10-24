<script lang="ts">
  import {
    windows,
    focusPrefix,
    moveWindow,
    closeWindow,
    canvasOnly,
  } from "../lib/canvas";
  import { nativePointerDown, stopNative } from "../lib/actions";
  import { connection } from "../lib/connection";
  import { setAppearance } from "../lib/appearance";
  import { t } from "../lib/i18n";

  export let path: string; // full file path (without the "img:" key prefix)
  export let zoom = 1;

  $: rect = $windows.get(key());
  $: focused = $focusPrefix === key();
  $: name = path.split("/").pop() || path;
  $: url = `${connection.apiBase}/api/file?path=${encodeURIComponent(path)}`;

  function key() {
    return "img:" + path;
  }

  let loaded = false;
  let natural = { w: 0, h: 0 };

  // --- header drag ---
  let drag: { px: number; py: number; x: number; y: number } | null = null;
  function headerDown(e: PointerEvent) {
    if (!rect) return;
    focusPrefix.set(key());
    drag = { px: e.clientX, py: e.clientY, x: rect.x, y: rect.y };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function headerMove(e: PointerEvent) {
    if (!drag || !rect) return;
    moveWindow(key(), {
      ...rect,
      x: Math.round(drag.x + (e.clientX - drag.px) / zoom),
      y: Math.round(drag.y + (e.clientY - drag.py) / zoom),
    });
  }
  function headerUp() {
    drag = null;
  }
</script>

{#if rect}
  <div
    class="absolute flex flex-col overflow-hidden rounded-lg border shadow-2xl {focused
      ? 'border-slate-500'
      : 'border-slate-800 opacity-90'}"
    class:pointer-events-none={$canvasOnly}
    style:left="{rect.x}px"
    style:top="{rect.y}px"
    style:width="{rect.w}px"
    style:height="{rect.h}px"
    use:nativePointerDown={() => focusPrefix.set(key())}
    use:stopNative={["dblclick"]}
    role="presentation"
  >
    <div
      class="pointer-events-auto flex h-8 shrink-0 cursor-grab select-none items-center gap-2 border-b border-slate-800 bg-[#1f2233] px-2 active:cursor-grabbing"
      role="presentation"
      use:nativePointerDown={headerDown}
      on:pointermove={headerMove}
      on:pointerup={headerUp}
      on:pointercancel={headerUp}
    >
      <span class="shrink-0 text-slate-500">🖼</span>
      <span class="min-w-0 flex-1 truncate text-xs font-medium text-slate-300" title={path}>
        {name}
      </span>
      <button
        class="shrink-0 rounded px-1 text-[10px] text-slate-500 hover:bg-slate-700 hover:text-slate-200"
        title={$t("setCanvasBg")}
        use:stopNative={["pointerdown"]}
        on:mousedown={(e) => e.button === 0 && setAppearance({ canvasBg: path })}
      >
        {$t("canvasBtn")}
      </button>
      <button
        class="shrink-0 rounded px-1 text-[10px] text-slate-500 hover:bg-slate-700 hover:text-slate-200"
        title={$t("setTermBg")}
        use:stopNative={["pointerdown"]}
        on:mousedown={(e) => e.button === 0 && setAppearance({ termBg: path })}
      >
        {$t("termBtn")}
      </button>
      {#if loaded && natural.w > 0}
        <span class="shrink-0 rounded bg-slate-800 px-1 text-[10px] tabular-nums text-slate-400">
          {natural.w}×{natural.h}
        </span>
      {/if}
      <a
        class="shrink-0 rounded px-1 text-xs text-slate-500 hover:text-slate-200"
        title={$t("download")}
        href={url}
        download={name}
        use:stopNative={["pointerdown"]}
      >
        ⇩
      </a>
      <button
        class="shrink-0 rounded px-1.5 text-sm text-slate-500 hover:text-red-400"
        title={$t("closeWin")}
        use:stopNative={["pointerdown", "dblclick"]}
        on:mousedown={(e) => e.button === 0 && closeWindow(key())}
      >
        ×
      </button>
    </div>

    <div class="min-h-0 flex-1 bg-[#101018]">
      <img
        src={url}
        alt={name}
        class="h-full w-full object-contain"
        on:load={() => (loaded = true)}
        bind:naturalWidth={natural.w}
        bind:naturalHeight={natural.h}
      />
    </div>
  </div>
{/if}
