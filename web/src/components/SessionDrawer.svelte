<script lang="ts">
  import { discovery } from "../lib/discovery";
  import { openWindow, focusPrefix, windows, drawerOpen } from "../lib/canvas";
  import { connection } from "../lib/connection";

  const list = discovery.list;
  const connState = connection.state;

  let creating = false;

  function stateColor(state: string): string {
    return state === "working"
      ? "bg-green-400"
      : state === "waiting"
        ? "bg-yellow-400"
        : "bg-slate-500";
  }

  /** Shorten a herdr terminal id for display. */
  function shortId(sid: string): string {
    return sid.length > 18 ? sid.slice(0, 15) + "…" : sid;
  }

  function createTerminal() {
    if (creating || $connState !== "connected") return;
    creating = true;
    connection.createSession("web", undefined, (sid) => {
      creating = false;
      openWindow(sid);
    });
    // Safety: if no reply arrives, re-enable after a while.
    setTimeout(() => (creating = false), 5000);
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
      <button
        class="flex w-full items-center justify-center gap-1 rounded-lg border border-dashed border-slate-600 px-2 py-1.5 text-sm text-slate-300 hover:bg-slate-800 disabled:opacity-40"
        disabled={$connState !== "connected" || creating}
        title="在 herdr 里新建一个终端"
        on:click={createTerminal}
      >
        <span class="text-base leading-none">+</span>
        {creating ? "创建中…" : "新建终端"}
      </button>
      {#each $list as ins (ins.sid)}
        <button
          class="flex w-full items-center gap-2 rounded-lg border px-2 py-1.5 text-left text-sm {$focusPrefix ===
          ins.sid
            ? "border-blue-500 bg-blue-500/10"
            : $windows.has(ins.sid)
              ? "border-slate-600 bg-slate-800/60"
              : "border-slate-700 hover:bg-slate-800"}"
          on:click={() => openWindow(ins.sid)}
        >
          <span class="h-2 w-2 shrink-0 rounded-full {stateColor(ins.state)}"></span>
          <span class="min-w-0 flex-1 truncate text-slate-200">
            {ins.title || shortId(ins.sid)}
          </span>
          {#if $windows.has(ins.sid)}
            <span class="shrink-0 rounded bg-slate-700 px-1 text-[10px] text-slate-300">画布中</span>
          {/if}
          {#if ins.state}
            <span
              class="shrink-0 text-[10px] {ins.state === 'waiting' ? 'text-yellow-400' : 'text-green-400'}"
            >
              {ins.state}
            </span>
          {/if}
        </button>
      {:else}
        <p class="px-2 py-4 text-center text-xs text-slate-500">
          {#if $connState === "connected"}
            没有在线会话——点上面「新建终端」开一个
          {:else if $connState === "connecting" || $connState === "reconnecting"}
            正在连接网关…
          {:else}
            未连接网关——点顶部状态栏配置
          {/if}
        </p>
      {/each}
    </div>
  </div>
{/if}
