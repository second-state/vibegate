<script lang="ts">
  import { tick } from "svelte";
  import {
    focusPrefix,
    getSession,
    zoomController,
    canvasZoom,
    canvasMoved,
    windows,
    IMAGE_KEY_PREFIX,
  } from "../lib/canvas";
  import { stickyMods, toggleMod, clearMods, applyMods } from "../lib/mods";

  // Floating bar pinned below the focused terminal window, in screen
  // coordinates. The position is measured from the window's live DOM rect
  // (getBoundingClientRect already includes the canvas transform), so canvas
  // zoom/pan moves the bar but never resizes it.
  let vw = 0;
  let vh = 0;
  let barW = 0;
  let barH = 0;
  let winLeft = 0;
  let winBottom = 0;
  const clamp = (v: number, lo: number, hi: number) =>
    Math.min(Math.max(v, lo), Math.max(lo, hi));

  /** Focused window — terminals only (image windows have no keys). */
  $: focusedRect =
    $focusPrefix && !$focusPrefix.startsWith(IMAGE_KEY_PREFIX)
      ? ($windows.get($focusPrefix) ?? null)
      : null;

  async function syncFromDom() {
    await tick();
    const el = document.querySelector<HTMLElement>("[data-focused-window]");
    const r = el?.getBoundingClientRect();
    winLeft = r ? r.left : 0;
    winBottom = r ? r.bottom : 0;
  }
  // Re-anchor on: focus/rect change, any canvas pan or zoom frame, viewport
  // resize. canvasMoved bumps on every transform frame, so the bar follows
  // the window while the canvas moves.
  $: syncTrigger = [focusedRect, $canvasMoved, $canvasZoom, vw, vh];
  $: if (syncTrigger) syncFromDom();

  // Top edge flush with the window's bottom edge.
  $: barX = Math.round(clamp(winLeft, 8, vw - barW - 8));
  $: barY = Math.round(clamp(winBottom, 8, vh - barH - 8));

  /** Quick keys: label → raw bytes sent to `pty_in`. */
  const KEYS_ROW1: [string, string][] = [
    ["Esc", "\x1b"],
    ["Tab", "\t"],
    ["⏎", "\r"],
    ["⌫", "\x7f"],
  ];

  /** Arrow cluster in keyboard layout (inverted T): ↑ on top, ← ↓ → below. */
  const ARROWS: { label: string; row: number; col: number; bytes: string }[] = [
    { label: "↑", row: 1, col: 2, bytes: "\x1b[A" },
    { label: "←", row: 2, col: 1, bytes: "\x1b[D" },
    { label: "↓", row: 2, col: 2, bytes: "\x1b[B" },
    { label: "→", row: 2, col: 3, bytes: "\x1b[C" },
  ];

  /** Sticky modifiers live in lib/mods.ts so the soft-keyboard input path
   *  (TerminalSession.#onData) consumes them too. */

  const modCls =
    "h-8 min-w-8 shrink-0 touch-manipulation rounded-md px-2 text-sm font-medium ";
  const modOff = modCls + "bg-slate-800 text-slate-300 hover:bg-slate-700";
  const modOn = modCls + "bg-blue-600 text-white";

  /** Encode bytes with xterm-style modifier parameters (see lib/mods.ts). */

  function press(bytes: string) {
    const prefix = $focusPrefix;
    const session = prefix ? getSession(prefix) : null;
    if (!session) return;
    const m = $stickyMods;
    const any = m.ctrl || m.alt || m.shift;
    session.sendRaw(any ? applyMods(bytes, m) : bytes);
    if (any) clearMods();
  }

  function doScroll(up: boolean) {
    const prefix = $focusPrefix;
    const session = prefix ? getSession(prefix) : null;
    session?.scroll(up);
  }

  /** Reset the canvas zoom to 1x, keeping the current view position. */
  function resetZoom() {
    const zt = $zoomController;
    if (!zt) return;
    zt.moveTo(zt.center, 1);
  }

  const keyCls =
    "h-8 min-w-8 shrink-0 touch-manipulation rounded-md bg-slate-800 px-2 text-sm text-slate-200 hover:bg-slate-700 active:bg-slate-600";
</script>

<svelte:window bind:innerWidth={vw} bind:innerHeight={vh} />

{#if focusedRect}
  <div
    class="fixed z-[5] rounded-lg border border-slate-800 bg-[#16161e]/95 shadow-xl backdrop-blur"
    style:left="{barX}px"
    style:top="{barY}px"
    bind:clientWidth={barW}
    bind:clientHeight={barH}
  >
    <div class="flex items-start gap-3 p-1.5">
      <!-- arrows: inverted T, like a real keyboard -->
      <div class="grid shrink-0 grid-cols-3 grid-rows-2 gap-0.5">
        {#each ARROWS as a (a.label)}
          <button
            class="{keyCls} px-0 text-center"
            style="grid-row:{a.row};grid-column:{a.col}"
            on:pointerdown|preventDefault={() => press(a.bytes)}
          >
            {a.label}
          </button>
        {/each}
      </div>

      <!-- the rest, in two rows + a scroll/zoom column -->
      <div class="flex min-w-0 flex-1 items-start gap-3">
        <div class="flex min-w-0 flex-1 flex-col gap-1">
          <div class="flex gap-1 overflow-x-auto">
            {#each KEYS_ROW1 as [label, bytes] (label)}
              <button
                class={keyCls}
                on:pointerdown|preventDefault={() => press(bytes)}
              >
                {label}
              </button>
            {/each}
          </div>
          <div class="flex gap-1 overflow-x-auto">
            <button
              class={$stickyMods.ctrl ? modOn : modOff}
              title="粘滞 Ctrl:下一次按键(按钮或软键盘输入)附带 Ctrl"
              on:pointerdown|preventDefault={() => toggleMod("ctrl")}
            >
              Ctrl
            </button>
            <button
              class={$stickyMods.alt ? modOn : modOff}
              title="粘滞 Alt:下一次按键(按钮或软键盘输入)附带 Alt"
              on:pointerdown|preventDefault={() => toggleMod("alt")}
            >
              Alt
            </button>
            <button
              class={$stickyMods.shift ? modOn : modOff}
              title="粘滞 Shift:下一次按键(按钮或软键盘输入)附带 Shift"
              on:pointerdown|preventDefault={() => toggleMod("shift")}
            >
              Shift
            </button>
            <button
              class="{Math.abs($canvasZoom - 1) > 0.01 ? modOn : modOff}"
              title="重置画布缩放到 1×(当前 {$canvasZoom.toFixed(1)}×)"
              on:pointerdown|preventDefault={resetZoom}
            >
              1×
            </button>
          </div>
        </div>
        <!-- scroll column: ⤒ over ⤓, always aligned -->
        <div class="flex shrink-0 flex-col items-center gap-0.5">
          <button
            class="{keyCls} w-8 px-0 font-bold"
            title="向上滚动服务端 scrollback"
            on:pointerdown|preventDefault={() => doScroll(true)}
          >
            ⤒
          </button>
          <button
            class="{keyCls} w-8 px-0 font-bold"
            title="向下滚动服务端 scrollback"
            on:pointerdown|preventDefault={() => doScroll(false)}
          >
            ⤓
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
