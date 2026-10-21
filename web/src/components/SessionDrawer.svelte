<script lang="ts">
  import { discovery } from "../lib/discovery";
  import { openWindow, focusPrefix, windows, drawerOpen } from "../lib/canvas";
  import { connection } from "../lib/connection";

  const list = discovery.list;
  const connState = connection.state;

  function stateColor(state: string): string {
    return state === "working"
      ? "bg-green-400"
      : state === "waiting"
        ? "bg-yellow-400"
        : "bg-slate-500";
  }

  /** Shorten a prefix like "root/1a2b…/12345/vibetty" for display. */
  function shortPrefix(p: string): string {
    const segs = p.split("/");
    return segs.length >= 4 ? `${segs[0]}/${segs[1].slice(0, 6)}…/${segs[2]}` : p;
  }
</script>

{#if $drawerOpen}
  <div
    class="absolute right-2 top-2 z-10 w-72 max-w-[calc(100vw-1rem)] overflow-hidden rounded-xl border border-slate-700 bg-slate-900/95 shadow-2xl backdrop-blur"
  >
    <div class="flex items-center justify-between border-b border-slate-800 px-3 py-2">
      <span class="text-xs font-semibold text-slate-300">在线会话</span>
      <span class="text-xs text-slate-500">{$list.length}</span>
    </div>
    <div class="flex max-h-[50vh] flex-col gap-1 overflow-auto p-2">
      {#each $list as ins (ins.prefix)}
        <button
          class="flex w-full items-center gap-2 rounded-lg border px-2 py-1.5 text-left text-sm {$focusPrefix ===
          ins.prefix
            ? "border-blue-500 bg-blue-500/10"
            : $windows.has(ins.prefix)
              ? "border-slate-600 bg-slate-800/60"
              : "border-slate-700 hover:bg-slate-800"}"
          on:click={() => openWindow(ins.prefix)}
        >
          <span class="h-2 w-2 shrink-0 rounded-full {stateColor(ins.state)}"></span>
          <span class="min-w-0 flex-1 truncate text-slate-200">
            {ins.title || shortPrefix(ins.prefix)}
          </span>
          {#if $windows.has(ins.prefix)}
            <span class="shrink-0 rounded bg-slate-700 px-1 text-[10px] text-slate-300">画布中</span>
          {/if}
          {#if ins.state}
            <span class="shrink-0 text-[10px] {ins.state === 'waiting' ? 'text-yellow-400' : 'text-green-400'}">
              {ins.state}
            </span>
          {/if}
        </button>
      {:else}
        <p class="px-2 py-6 text-center text-xs text-slate-500">
          {#if $connState === "connected"}
            等待 presence 发现实例…
            <br />确保有 vibetty 正连着同一个 broker
          {:else if $connState === "connecting" || $connState === "reconnecting"}
            正在连接 broker…
          {:else}
            未连接 broker——点顶部状态栏配置
          {/if}
        </p>
      {/each}
    </div>
  </div>
{/if}
