<script lang="ts">
  import { onMount } from "svelte";
  import { TerminalSession } from "../lib/session";
  import {
    registerSession,
    unregisterSession,
    windows,
    focusPrefix,
    moveWindow,
    closeWindow,
    MIN_W,
    MIN_H,
  } from "../lib/canvas";
  import { discovery } from "../lib/discovery";

  export let prefix: string;
  export let zoom = 1;

  let session: TerminalSession | null = null;
  let termEl: HTMLElement;
  let readOnly = false;

  const instances = discovery.instances;

  $: rect = $windows.get(prefix);
  $: instance = $instances.get(prefix);
  $: focused = $focusPrefix === prefix;

  // Refit when the window is resized (initial fit happens in attach()).
  let lastW = 0;
  let lastH = 0;
  $: if (rect && session && (rect.w !== lastW || rect.h !== lastH)) {
    const resized = lastW !== 0;
    lastW = rect.w;
    lastH = rect.h;
    if (resized) session.fit();
  }

  // Focus drives bandwidth: only the focused window gets pushed data.
  $: if (session) session.setActive(focused);

  onMount(() => {
    session = new TerminalSession(prefix);
    registerSession(prefix, session);
    session.attach(termEl);
    return () => {
      unregisterSession(prefix);
      session!.close();
      session = null;
    };
  });

  // --- header drag ---
  let drag: { px: number; py: number; x: number; y: number } | null = null;
  function headerDown(e: PointerEvent) {
    if (!rect) return;
    focusPrefix.set(prefix);
    drag = { px: e.clientX, py: e.clientY, x: rect.x, y: rect.y };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function headerMove(e: PointerEvent) {
    if (!drag || !rect) return;
    moveWindow(prefix, {
      ...rect,
      x: Math.round(drag.x + (e.clientX - drag.px) / zoom),
      y: Math.round(drag.y + (e.clientY - drag.py) / zoom),
    });
  }
  function headerUp() {
    drag = null;
  }

  // --- corner resize ---
  let resizing: { px: number; py: number; w: number; h: number } | null = null;
  let resizeTimer: ReturnType<typeof setTimeout> | null = null;
  function resizeDown(e: PointerEvent) {
    if (!rect) return;
    focusPrefix.set(prefix);
    e.stopPropagation();
    resizing = { px: e.clientX, py: e.clientY, w: rect.w, h: rect.h };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function resizeMove(e: PointerEvent) {
    if (!resizing || !rect) return;
    moveWindow(prefix, {
      ...rect,
      w: Math.max(MIN_W, Math.round(resizing.w + (e.clientX - resizing.px) / zoom)),
      h: Math.max(MIN_H, Math.round(resizing.h + (e.clientY - resizing.py) / zoom)),
    });
    if (!resizeTimer) {
      resizeTimer = setTimeout(() => {
        resizeTimer = null;
        session?.fit();
      }, 150);
    }
  }
  function resizeUp() {
    if (resizeTimer) {
      clearTimeout(resizeTimer);
      resizeTimer = null;
    }
    session?.fit();
    resizing = null;
  }

  function toggleReadOnly() {
    readOnly = !readOnly;
    if (session) session.readOnly = readOnly;
    focusPrefix.set(prefix);
  }

  // Wheel over the focused terminal scrolls its own scrollback instead of
  // panning the canvas; unfocused windows let the canvas pan (same as sshx).
  function wheel(e: WheelEvent) {
    if (focused) e.stopPropagation();
  }

  $: title = instance?.title || prefix;
  $: state = instance?.state ?? "";
  $: format = instance?.format ?? "text";
</script>

{#if rect}
  <div
    class="absolute flex flex-col overflow-hidden rounded-lg border shadow-2xl {focused
      ? 'border-slate-500'
      : 'border-slate-800 opacity-90'}"
    style:left="{rect.x}px"
    style:top="{rect.y}px"
    style:width="{rect.w}px"
    style:height="{rect.h}px"
    on:pointerdown|stopPropagation={() => focusPrefix.set(prefix)}
    on:dblclick|stopPropagation
    role="presentation"
  >
    <!-- Header / drag handle -->
    <div
      class="flex h-8 shrink-0 cursor-grab select-none items-center gap-2 border-b border-slate-800 bg-[#1f2233] px-2 active:cursor-grabbing"
      role="presentation"
      on:pointerdown={headerDown}
      on:pointermove={headerMove}
      on:pointerup={headerUp}
      on:pointercancel={headerUp}
    >
      <span
        class="h-2 w-2 shrink-0 rounded-full {state === 'working'
          ? 'bg-green-400'
          : state === 'waiting'
            ? 'bg-yellow-400'
            : 'bg-slate-500'}"
        title={state || 'unknown'}
      ></span>
      <span class="min-w-0 flex-1 truncate text-xs font-medium text-slate-300" title={prefix}>
        {title}
      </span>
      {#if format !== "text"}
        <span class="shrink-0 rounded bg-orange-900 px-1 text-[10px] text-orange-300">
          format:{format} 不支持
        </span>
      {/if}
      <button
        class="shrink-0 rounded px-1 text-xs {readOnly ? 'text-yellow-400' : 'text-slate-500 hover:text-slate-300'}"
        title={readOnly ? "只读模式(点击开启键盘输入)" : "点击切换为只读模式"}
        on:pointerdown|stopPropagation
        on:click={toggleReadOnly}
      >
        👁
      </button>
      <button
        class="shrink-0 rounded px-1.5 text-sm text-slate-500 hover:text-red-400"
        title="关闭窗口"
        on:pointerdown|stopPropagation
        on:click={() => closeWindow(prefix)}
      >
        ×
      </button>
    </div>

    <!-- Terminal -->
    <div class="min-h-0 flex-1 bg-[#1a1b26]">
      <div bind:this={termEl} class="h-full w-full" on:wheel={wheel}></div>
    </div>

    {#if readOnly && focused}
      <div class="pointer-events-none absolute inset-x-0 top-8 flex justify-center">
        <span class="rounded-b bg-yellow-900/80 px-2 py-0.5 text-[10px] text-yellow-200">只读</span>
      </div>
    {/if}

    <!-- Resize handle -->
    <div
      class="absolute bottom-0 right-0 h-4 w-4 cursor-nwse-resize touch-none"
      role="presentation"
      on:pointerdown={resizeDown}
      on:pointermove={resizeMove}
      on:pointerup={resizeUp}
      on:pointercancel={resizeUp}
    ></div>
  </div>
{/if}
