<script lang="ts">
  import { writable } from "svelte/store";
  import { focusPrefix, getSession, zoomController, canvasZoom } from "../lib/canvas";

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

  /** Sticky modifiers: click to latch (highlighted), the next key includes
   *  them, then they clear. */
  type Mods = { ctrl: boolean; alt: boolean; shift: boolean };
  const sticky = writable<Mods>({ ctrl: false, alt: false, shift: false });

  function toggleMod(k: keyof Mods) {
    sticky.update((m) => ({ ...m, [k]: !m[k] }));
  }

  const modCls =
    "h-8 min-w-8 shrink-0 touch-manipulation rounded-md px-2 text-sm font-medium ";
  const modOff = modCls + "bg-slate-800 text-slate-300 hover:bg-slate-700";
  const modOn = modCls + "bg-blue-600 text-white";

  /** Encode bytes with xterm-style modifier parameters. */
  function applyMods(bytes: string, m: Mods): string {
    const mod = 1 + (m.shift ? 1 : 0) + (m.alt ? 2 : 0) + (m.ctrl ? 4 : 0);
    // Arrow keys: CSI 1;<mod><letter>
    const csi = /^(\x1b\[)([ABCD])$/.exec(bytes);
    if (csi) return `\x1b[1;${mod}${csi[2]}`;
    switch (bytes) {
      case "\t":
        return m.shift && !m.alt && !m.ctrl ? "\x1b[Z" : `\x1b[27;${mod}~`;
      case "\r":
      case "\x7f":
        return `\x1b[27;${mod}~`;
      case "\x1b":
        return "\x1b\x1b";
      default: {
        // Single printable char: ctrl maps to control codes, alt = ESC prefix.
        if (bytes.length === 1) {
          let out = bytes;
          if (m.ctrl) {
            const code = out.toUpperCase().charCodeAt(0);
            if (code >= 64 && code <= 95) out = String.fromCharCode(code & 0x1f);
          }
          return (m.alt ? "\x1b" : "") + out;
        }
        return bytes;
      }
    }
  }

  function press(bytes: string) {
    const prefix = $focusPrefix;
    const session = prefix ? getSession(prefix) : null;
    if (!session) return;
    let v: Mods = { ctrl: false, alt: false, shift: false };
    sticky.subscribe((m) => (v = m))();
    const any = v.ctrl || v.alt || v.shift;
    session.sendRaw(any ? applyMods(bytes, v) : bytes);
    if (any) sticky.set({ ctrl: false, alt: false, shift: false });
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
              class={$sticky.ctrl ? modOn : modOff}
              title="粘滞 Ctrl:下一次按键附带 Ctrl"
              on:pointerdown|preventDefault={() => toggleMod("ctrl")}
            >
              Ctrl
            </button>
            <button
              class={$sticky.alt ? modOn : modOff}
              title="粘滞 Alt:下一次按键附带 Alt"
              on:pointerdown|preventDefault={() => toggleMod("alt")}
            >
              Alt
            </button>
            <button
              class={$sticky.shift ? modOn : modOff}
              title="粘滞 Shift:下一次按键附带 Shift"
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
