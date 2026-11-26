<script lang="ts">
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { TerminalSession, type SessionStatus } from "../lib/session";
  import {
    registerSession,
    unregisterSession,
    windows,
    focusPrefix,
    moveWindow,
    closeWindow,
    canvasOnly,
    filesNavigate,
    restoredKeys,
    MIN_W,
    MIN_H,
  } from "../lib/canvas";
  import { discovery } from "../lib/discovery";
import { stopNative, nativePointerDown } from "../lib/actions";
  import { connection } from "../lib/connection";
  import { appearance, resolveBgUrl } from "../lib/appearance";
  import { TERM_PRESETS } from "../lib/theme";
  import { t } from "../lib/i18n";
  import { statusColor } from "../lib/discovery";
  import { termShotDataURL } from "../lib/termshot";
  import { openImageBlob } from "../lib/canvas";
  import { copyTextToClipboard, showCopiedToast } from "../lib/session";

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
  // blip cannot wipe windows) → remove the window from the canvas. Restored
  // windows are exempt: at startup "ended" + pane list not yet received is
  // the normal pre-reconnect state, not a dead pane.
  $: if (
    (status === "ended" || status === "error") &&
    $connState === "connected" &&
    rect &&
    !instance &&
    !restoredKeys.has(prefix)
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
    const unsubStatus = session.status.subscribe((s) => {
      status = s;
      if (s === "live") restoredKeys.delete(prefix); // connected: behave normally again
    });
    const unsubSize = session.size.subscribe((s) => (size = s));
    // Restored-from-layout windows stay disconnected: the ended-mask shows
    // and a click reconnects. Freshly opened windows attach immediately.
    session.attach(termEl, restoredKeys.has(prefix));

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

    // Right-click: block the button-2 report to the app and open our menu.
    const trackCell = (e: MouseEvent) => {
      // xterm: inner .xterm-screen; ghostty-web (spike): the element itself
      // (canvas fills it), same geometry.
      const scr =
        session?.term.element?.querySelector(".xterm-screen") ?? session?.term.element;
      if (!scr || !session) return;
      const b = scr.getBoundingClientRect();
      const cw = session.term.cols ? b.width / session.term.cols : 0;
      const ch = session.term.rows ? b.height / session.term.rows : 0;
      lastCell = {
        col: Math.min(session.term.cols - 1, Math.max(0, Math.floor((e.clientX - b.left) / cw))),
        row: Math.min(session.term.rows - 1, Math.max(0, Math.floor((e.clientY - b.top) / ch))),
      };
    };
    const rightDownBlock = (e: MouseEvent) => {
      if (e.button !== 2) return;
      e.preventDefault(); // no native menu
      e.stopPropagation(); // app never sees the button-2 press
    };
    const ctxOpenAt = (e: MouseEvent) => {
      if (e.button !== 2) return;
      e.preventDefault();
      openCtxMenu(e.clientX, e.clientY);
    };
    termEl.addEventListener("mousemove", trackCell);
    termEl.addEventListener("mousedown", rightDownBlock, true);
    termEl.addEventListener("contextmenu", ctxOpenAt);

    return () => {
      termEl.removeEventListener("keydown", guard);
      termEl.removeEventListener("touchstart", touchStart);
      termEl.removeEventListener("touchmove", touchMove);
      termEl.removeEventListener("touchend", touchEnd);
      termEl.removeEventListener("mousemove", trackCell);
      termEl.removeEventListener("mousedown", rightDownBlock, true);
      termEl.removeEventListener("contextmenu", ctxOpenAt);
      closeCtxMenu();
      unsubStatus();
      unsubSize();
      unregisterSession(prefix);
      session!.close();
      session = null;
    };
  });

  // --- terminal context menu ---
  let ctxMenu: { x: number; y: number } | null = null;
  let lastCell: { col: number; row: number } | null = null;
  const canPaste = typeof navigator !== "undefined" && !!navigator.clipboard;

  function openCtxMenu(x: number, y: number) {
    ctxMenu = {
      x: Math.min(x, window.innerWidth - 170),
      y: Math.min(y, window.innerHeight - 160),
    };
    document.addEventListener("mousedown", closeCtxOnOutside, true);
    document.addEventListener("keydown", closeCtxOnEsc, true);
  }
  function closeCtxMenu() {
    ctxMenu = null;
    document.removeEventListener("mousedown", closeCtxOnOutside, true);
    document.removeEventListener("keydown", closeCtxOnEsc, true);
  }
  function closeCtxOnOutside(e: MouseEvent) {
    const t = e.target as HTMLElement;
    if (!t.closest("[data-ctx-menu]")) closeCtxMenu();
  }
  function closeCtxOnEsc(e: KeyboardEvent) {
    if (e.key === "Escape") closeCtxMenu();
  }
  /** Render the menu at body level: fixed positioning inside the transformed
   *  world container would be scaled/offset away from the cursor. */
  function portalToBody(node: HTMLElement) {
    document.body.appendChild(node);
  }
  /** Open the file drawer at this terminal's working directory. */
  function ctxOpenWorkdir() {
    closeCtxMenu();
    filesNavigate.set(instance?.cwd || "");
  }

  /** Screenshot this terminal's visible grid: vector-rendered from the
   *  xterm buffer, opened in an in-memory image window (nothing stored on
   *  the gateway machine). */
  function ctxScreenshot() {
    closeCtxMenu();
    if (!session) return;
    termShotDataURL(session, {
      chrome: $appearance.chrome,
      title: instance?.title || prefix,
      status: state,
      header: false,
    }).then((shot) => {
      if (!shot) return;
      const ts = new Date();
      const p2 = (n: number) => String(n).padStart(2, "0");
      const name = `term-${ts.getFullYear()}${p2(ts.getMonth() + 1)}${p2(ts.getDate())}-${p2(ts.getHours())}${p2(ts.getMinutes())}${p2(ts.getSeconds())}.png`;
      openImageBlob(name, shot.dataUrl, shot.w + 16, shot.h + 16);
    });
  }

  async function ctxPaste() {
    closeCtxMenu();
    try {
      const text = await navigator.clipboard.readText();
      if (text) session?.sendRaw(text);
    } catch (e) {
      showCopiedToast(false, String(e));
    }
  }
  function ctxSendRightClick() {
    const c = lastCell ?? { col: 0, row: 0 };
    // SGR mouse report: button 2 press (M) + release (m)
    session?.sendRaw(`\x1b[<2;${c.col + 1};${c.row + 1}M\x1b[<2;${c.col + 1};${c.row + 1}m`);
    closeCtxMenu();
  }
  function ctxToggleCanvasMode() {
    canvasOnly.update((v) => !v);
    closeCtxMenu();
  }
  /** Copy this pane's herdr pane id (workspace:pane, e.g. "w7:p1") — the id
   *  the herdr CLI uses. Note the window prefix is the terminal id, so look
   *  the pane id up from the pane list. */
  function ctxCopyPaneId() {
    closeCtxMenu();
    const pane = get(discovery.list).find((i) => i.sid === prefix);
    copyTextToClipboard(pane?.pane_id || prefix)
      .then(() => showCopiedToast(true, ""))
      .catch((e) => showCopiedToast(false, String(e)));
  }

  // --- header drag ---
  let drag: { px: number; py: number; x: number; y: number } | null = null;
  function headerDown(e: PointerEvent) {
    if (e.button !== 0) return; // right-drag belongs to the canvas
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
    if (e.button !== 0) return; // right-drag belongs to the canvas
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
    class:pointer-events-none={$canvasOnly}
    style:left="{rect.x}px"
    style:top="{rect.y}px"
    style:width="{rect.w}px"
    style:height="{rect.h}px"
    use:nativePointerDown={() => {
      focusPrefix.set(prefix);
      // The mousedown default action (running after all handlers) clears
      // focus to <body> — xterm preventDefaults its own events, but our
      // root listener sees the event too late for that to matter here.
      // Re-focus the helper textarea after the default action, so the
      // FIRST click enables keyboard input.
      setTimeout(() => session?.term.textarea?.focus(), 0);
    }}
    use:stopNative={["dblclick"]}
    role="presentation"
    data-focused-window={focused ? "1" : undefined}
    data-chrome={$appearance.chrome}
    data-window="1"
  >
    <!-- Header / drag handle -->
    <div
      class="win-head pointer-events-auto flex h-8 shrink-0 cursor-grab select-none items-center gap-2 border-b border-slate-800 bg-[#1f2233] px-2 active:cursor-grabbing"
      role="presentation"
      use:nativePointerDown={headerDown}
      on:pointermove={headerMove}
      on:pointerup={headerUp}
      on:pointercancel={headerUp}
    >
      {#if status === "ended" || status === "error"}
        <button
          class="shrink-0 rounded px-1 text-sm text-blue-400 hover:bg-slate-700"
          title={$t("reconnect")}
          use:stopNative={["pointerdown"]}
          on:mousedown={(e) => e.button === 0 && session?.reopen()}
        >
          ⟳
        </button>
      {/if}
      <span
        data-status-dot
        data-status={state || "shell"}
        class="h-2 w-2 shrink-0 rounded-full {statusColor(state)}"
        title={state || 'shell'}
      ></span>
      <span class="min-w-0 flex-1 truncate text-xs font-medium text-slate-300" title={prefix}>
        {title}
      </span>
      {#if size.cols > 0}
        <span
          class="shrink-0 rounded bg-slate-800 px-1 text-[10px] tabular-nums text-slate-400"
          title={$t("gridSize")}
        >
          {size.cols}×{size.rows}
        </span>
      {/if}
      <button
        class="shrink-0 rounded px-1.5 text-sm text-slate-500 hover:text-red-400"
        title={$t("closeWindow")}
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

    {#if (status === "ended" || status === "error") && instance}
      <!-- click anywhere on the mask reconnects -->
      <div
        class="absolute inset-0 top-8 z-10 flex items-center justify-center bg-[#16161e]/85"
        role="button"
        tabindex="0"
        title={$t("reconnect")}
        on:mousedown={(e) => e.button === 0 && session?.reopen()}
        on:keydown={(e) => (e.key === "Enter" || e.key === " ") && session?.reopen()}
      >
        <p class="text-xs text-slate-400">{$t("disconnectedHint")}</p>
      </div>
    {:else if status === "ended" || status === "error"}
      <div class="pointer-events-none absolute inset-0 top-8 z-10 flex items-center justify-center bg-[#16161e]/85">
        <p class="text-xs text-slate-400">{$t("paneGoneHint")}</p>
      </div>
    {/if}

    <!-- Resize handle -->
    <div
      class="pointer-events-auto absolute -bottom-1.5 -right-1.5 h-9 w-9 cursor-nwse-resize touch-none"
      role="presentation"
      use:nativePointerDown={resizeDown}
      on:pointermove={resizeMove}
      on:pointerup={resizeUp}
      on:pointercancel={resizeUp}
    ></div>
  </div>
{/if}

{#if ctxMenu}
  <div
    data-ctx-menu
    use:portalToBody
    class="fixed z-[60] min-w-[170px] overflow-hidden rounded-md border border-slate-700 bg-[#1f2233] py-1 shadow-2xl"
    style:left="{ctxMenu.x}px"
    style:top="{ctxMenu.y}px"
  >
    {#if canPaste}
      <button
        class="block w-full px-3 py-1.5 text-left text-xs text-slate-200 hover:bg-slate-700"
        on:mousedown|preventDefault={ctxPaste}
      >
        {$t("ctxPaste")}
      </button>
    {/if}
    <button
      class="block w-full px-3 py-1.5 text-left text-xs text-slate-200 hover:bg-slate-700"
      title={instance?.cwd || ""}
      on:mousedown|preventDefault={ctxOpenWorkdir}
    >
      {$t("ctxOpenWorkdir")}
    </button>
    <button
      class="block w-full px-3 py-1.5 text-left text-xs text-slate-200 hover:bg-slate-700"
      on:mousedown|preventDefault={ctxScreenshot}
    >
      {$t("ctxScreenshot")}
    </button>
    <button
      class="block w-full px-3 py-1.5 text-left text-xs text-slate-200 hover:bg-slate-700"
      on:mousedown|preventDefault={ctxCopyPaneId}
    >
      {$t("ctxCopyPaneId")}
    </button>
    <button
      class="block w-full px-3 py-1.5 text-left text-xs text-slate-200 hover:bg-slate-700"
      on:mousedown|preventDefault={ctxSendRightClick}
    >
      {$t("ctxSendRight")}
    </button>
    <button
      class="block w-full px-3 py-1.5 text-left text-xs text-slate-200 hover:bg-slate-700"
      on:mousedown|preventDefault={ctxToggleCanvasMode}
    >
      {$t("ctxToggleCanvas")}
    </button>
  </div>
{/if}
