<script lang="ts">
  import { focusPrefix, getSession, zoomController, canvasZoom } from "../lib/canvas";
  import { stickyMods, toggleMod, clearMods, applyMods } from "../lib/mods";

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

{#if $focusPrefix}
  <div class="z-20 border-b border-slate-800 bg-[#16161e] px-2 py-1">
    <div class="flex items-start gap-3">
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
