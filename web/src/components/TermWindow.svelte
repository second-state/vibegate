<script lang="ts">
  import { onMount } from "svelte";
  import { TerminalSession, type SessionStatus } from "../lib/session";
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
import { stopNative, nativePointerDown } from "../lib/actions";
  import { connection } from "../lib/connection";
  import { appearance, resolveBgUrl } from "../lib/appearance";
  import { TERM_PRESETS } from "../lib/theme";

  export let prefix: string;
  export let zoom = 1;

  let session: TerminalSession | null = null;
  let termEl: HTMLElement;
  let status: SessionStatus = "connecting";
  let size = { cols: 0, rows: 0 };

  const instances = discovery.list;

  $: rect = $windows.get(prefix);
  $: instance = $instances.find((i) => i.sid === prefix);
  $: focused = $focusPrefix === prefix;
  const connState = connection.state;

  // Pane gone from herdr (judged only while the gateway is connected, so a
  // blip cannot wipe windows) → remove the window from the canvas.
  $: if (
    (status === "ended" || status === "error") &&
    $connState === "connected" &&
    rect &&
    !instance
  ) {
    closeWindow(prefix);
  }

  // Refit when the window is resized (initial fit happens in attach()).
  let lastW = 0;
  let lastH = 0;
  $: if (rect && session && (rect.w !== lastW || rect.h !== lastH)) {
    const resized = lastW !== 0;
    lastW = rect.w;
    lastH = rect.h;
    if (resized) session.fit();
  }

  onMount(() => {
    session = new TerminalSession(prefix, connection.urlValue);
    registerSession(prefix, session);
    const unsubStatus = session.status.subscribe((s) => (status = s));
    const unsubSize = session.size.subscribe((s) => (size = s));
    session.attach(termEl);

    // Keep browser focus on the terminal: an unhandled Tab moves focus out of
    // xterm (in terminal-browser's electron shell the compositor then switches
    // windows — looks exactly like Alt+Tab). Preventing the default lets the
    // keydown still reach xterm, so the \t byte is sent as usual.
    const guard = (e: KeyboardEvent) => {
      if (e.key === "Tab" && !e.ctrlKey && !e.altKey && !e.metaKey) {
        e.preventDefault();
      }
    };
    termEl.addEventListener("keydown", guard, true);

    // Mobile swipe → PageUp/PageDown (wheel events don't fire for touch).
    let touchY: number | null = null;
    const touchStart = (e: TouchEvent) => {
      touchY = e.touches[0]?.clientY ?? null;
    };
    const touchMove = (e: TouchEvent) => {
      e.preventDefault(); // no overscroll/pull-refresh over the terminal
      if (touchY === null || !session) return;
      const y = e.touches[0]?.clientY;
      if (y === undefined) return;
      const dy = touchY - y; // >0 = swiping up
      if (Math.abs(dy) >= 40) {
        session.scroll(dy < 0); // swipe up → PageDown, swipe down → PageUp
        touchY = y;
      }
    };
    const touchEnd = () => (touchY = null);
    termEl.addEventListener("touchstart", touchStart);
    termEl.addEventListener("touchmove", touchMove, { passive: false });
    termEl.addEventListener("touchend", touchEnd);

    return () => {
      termEl.removeEventListener("keydown", guard);
      termEl.removeEventListener("touchstart", touchStart);
      termEl.removeEventListener("touchmove", touchMove);
      termEl.removeEventListener("touchend", touchEnd);
      unsubStatus();
      unsubSize();
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

  // Wheel over the focused terminal sends PageUp/PageDown to the remote pane
  // (throttled); unfocused windows let the canvas pan.
  let wheelPending = false;
  function wheel(e: WheelEvent) {
    if (!focused) return; // bubble → canvas pans
    e.preventDefault();
    e.stopPropagation();
    if (!session || wheelPending) return;
    wheelPending = true;
    session.scroll(e.deltaY < 0); // up = PageUp
    setTimeout(() => (wheelPending = false), 150);
  }

  $: title = instance?.title || prefix;
  $: state = instance?.state ?? "";

  // With a background image set, xterm's own background goes fully transparent
  // (see TerminalSession.#resolvedTheme) — this layer paints the theme's
  // background color underneath, and the image blends onto it by opacity.
  $: termBgColor = (
    TERM_PRESETS[$appearance.theme] ?? TERM_PRESETS["tokyo-night"]
  ).theme.background;
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
    use:nativePointerDown={() => focusPrefix.set(prefix)}
    use:stopNative={["dblclick"]}
    role="presentation"
    data-focused-window={focused ? "1" : undefined}
  >
    <!-- Header / drag handle -->
    <div
      class="flex h-8 shrink-0 cursor-grab select-none items-center gap-2 border-b border-slate-800 bg-[#1f2233] px-2 active:cursor-grabbing"
      role="presentation"
      use:nativePointerDown={headerDown}
      on:pointermove={headerMove}
      on:pointerup={headerUp}
      on:pointercancel={headerUp}
    >
      {#if status === "ended" || status === "error"}
        <button
          class="shrink-0 rounded px-1 text-sm text-blue-400 hover:bg-slate-700"
          title="重新连接"
          use:stopNative={["pointerdown"]}
          on:mousedown={(e) => e.button === 0 && session?.reopen()}
        >
          ⟳
        </button>
      {/if}
      <span
        class="h-2 w-2 shrink-0 rounded-full {state === 'working'
          ? 'bg-green-400'
          : state === 'waiting'
            ? 'bg-yellow-400'
            : 'bg-slate-500'}"
        title={state || 'shell'}
      ></span>
      <span class="min-w-0 flex-1 truncate text-xs font-medium text-slate-300" title={prefix}>
        {title}
      </span>
      {#if size.cols > 0}
        <span
          class="shrink-0 rounded bg-slate-800 px-1 text-[10px] tabular-nums text-slate-400"
          title="终端大小(列×行)"
        >
          {size.cols}×{size.rows}
        </span>
      {/if}
      <button
        class="shrink-0 rounded px-1.5 text-sm text-slate-500 hover:text-red-400"
        title="关闭窗口(不影响远端会话)"
        use:stopNative={["pointerdown", "dblclick"]}
        on:mousedown={(e) => e.button === 0 && closeWindow(prefix)}
      >
        ×
      </button>
    </div>

    <!-- Terminal (optional background image behind the transparent xterm) -->
    <div class="relative min-h-0 flex-1">
      {#if $appearance.termBg}
        <div
          class="pointer-events-none absolute inset-0"
          style:background={termBgColor}
        ></div>
        <div
          class="pointer-events-none absolute inset-0 bg-cover bg-center"
          style:background-image={`url("${resolveBgUrl($appearance.termBg)}")`}
          style:opacity={Math.min(100, Math.max(0, $appearance.termBgOpacity ?? 100)) / 100}
        ></div>
      {/if}
      <div bind:this={termEl} class="relative h-full w-full" on:wheel|capture={wheel}></div>
    </div>

    {#if status === "ended" || status === "error"}
      <div class="pointer-events-none absolute inset-0 top-8 z-10 flex items-center justify-center bg-[#16161e]/85">
        <p class="text-xs text-slate-400">
          {#if instance}连接已断开——点标题栏 ⟳ 重新连接{:else}远端面板已关闭,窗口即将移除…{/if}
        </p>
      </div>
    {/if}

    <!-- Resize handle -->
    <div
      class="absolute -bottom-1.5 -right-1.5 h-9 w-9 cursor-nwse-resize touch-none"
      role="presentation"
      use:nativePointerDown={resizeDown}
      on:pointermove={resizeMove}
      on:pointerup={resizeUp}
      on:pointercancel={resizeUp}
    ></div>
  </div>
{/if}
