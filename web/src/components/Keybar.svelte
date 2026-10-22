<script lang="ts">
  import { focusPrefix, getSession } from "../lib/canvas";

  /** Quick keys: label → raw bytes sent to `pty_in`. */
  const KEYS: [string, string][] = [
    ["Esc", "\x1b"],
    ["Tab", "\t"],
    ["↑", "\x1b[A"],
    ["↓", "\x1b[B"],
    ["←", "\x1b[D"],
    ["→", "\x1b[C"],
    ["⏎", "\r"],
    ["⌫", "\x7f"],
    ["^C", "\x03"],
    ["^D", "\x04"],
    ["^Z", "\x1a"],
  ];

  const keyCls =
    "h-9 min-w-9 shrink-0 touch-manipulation rounded-md bg-slate-800 px-2 text-sm text-slate-200 hover:bg-slate-700 active:bg-slate-600";

  function press(bytes: string) {
    const prefix = $focusPrefix;
    const session = prefix ? getSession(prefix) : null;
    session?.sendRaw(bytes);
  }

  function doScroll(up: boolean) {
    const prefix = $focusPrefix;
    const session = prefix ? getSession(prefix) : null;
    session?.scroll(up);
  }
</script>

{#if $focusPrefix}
  <div
    class="z-20 flex items-center gap-1 overflow-x-auto border-b border-slate-800 bg-[#16161e] px-2 py-1.5"
  >
    <button
      class="{keyCls} font-bold"
      title="向上滚动服务端 scrollback"
      on:pointerdown|preventDefault={() => doScroll(true)}
    >
      ⤒
    </button>
    <button
      class="{keyCls} font-bold"
      title="向下滚动服务端 scrollback"
      on:pointerdown|preventDefault={() => doScroll(false)}
    >
      ⤓
    </button>
    <span class="mx-1 h-5 w-px shrink-0 bg-slate-700"></span>
    {#each KEYS as [label, bytes] (label)}
      <button
        class={keyCls}
        on:pointerdown|preventDefault={() => press(bytes)}
      >
        {label}
      </button>
    {/each}
  </div>
{/if}
