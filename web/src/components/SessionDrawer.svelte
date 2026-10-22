<script lang="ts">
  import { discovery } from "../lib/discovery";
  import { openWindow, focusPrefix, windows, drawerOpen } from "../lib/canvas";
  import { writable } from "svelte/store";
  import { connection } from "../lib/connection";
  import type { Instance } from "../lib/discovery";

  const list = discovery.list;
  const connState = connection.state;
  const workspaces = connection.workspaces;

  /** Per-workspace collapse state; a store so the template stays reactive. */
  const expandedStore = writable<Record<string, boolean>>({});
  let creatingWorkspace = false;
  let creatingPane: string | null = null;

  const toggle = (id: string) =>
    expandedStore.update((m) => ({ ...m, [id]: m[id] === false }));

  function stateColor(state: string): string {
    return state === "working"
      ? "bg-green-400"
      : state === "waiting"
        ? "bg-yellow-400"
        : "bg-slate-500";
  }

  type Group = { id: string; label: string; number: number; panes: Instance[] };

  $: groups = buildGroups($workspaces, $list);
  $: orphans = $list.filter((p) => !groups.some((g) => g.id === p.workspace_id));

  function buildGroups(
    wss: { id: string; label: string; number: number }[],
    panes: Instance[],
  ): Group[] {
    return wss.map((w) => ({
      id: w.id,
      label: w.label || `workspace ${w.number}`,
      number: w.number,
      panes: panes.filter((p) => p.workspace_id === w.id),
    }));
  }

  function createWorkspace() {
    if (creatingWorkspace || $connState !== "connected") return;
    creatingWorkspace = true;
    connection.createWorkspace("web", undefined, (sid) => {
      creatingWorkspace = false;
      expandedStore.update((m) => ({ ...m, [sid]: true }));
      openWindow(sid);
    });
    setTimeout(() => (creatingWorkspace = false), 5000);
  }

  function createPane(groupId: string) {
    if (creatingPane || $connState !== "connected") return;
    creatingPane = groupId;
    expandedStore.update((m) => ({ ...m, [groupId]: true }));
    connection.createPane(groupId, undefined, (sid) => {
      creatingPane = null;
      openWindow(sid);
    });
    setTimeout(() => (creatingPane === groupId && (creatingPane = null)), 5000);
  }

  function killPane(e: { stopPropagation(): void }, paneId: string) {
    e.stopPropagation();
    connection.killPane(paneId);
  }
</script>

{#if $drawerOpen}
  <div
    class="absolute inset-y-2 right-2 z-10 flex w-72 max-w-[calc(100vw-1rem)] flex-col overflow-hidden rounded-xl border border-slate-700 bg-slate-900/95 shadow-2xl backdrop-blur"
  >
    <div class="flex items-center justify-between border-b border-slate-800 px-3 py-2">
      <span class="text-xs font-semibold text-slate-300">会话</span>
      <span class="text-xs text-slate-500">{$workspaces.length} ws · {$list.length} pane</span>
    </div>
    <div class="flex min-h-0 flex-1 flex-col overflow-auto p-2">
      <!-- workspace → pane tree -->
      {#each groups as g (g.id)}
        <div class="mb-0.5">
          <!-- workspace header -->
          <div
            class="group flex w-full items-center gap-1 rounded-lg px-1 py-1 text-xs text-slate-400 hover:bg-slate-800/70"
          >
            <button
              class="flex min-w-0 flex-1 items-center gap-1 text-left"
              on:click={() => toggle(g.id)}
              title="展开/收起"
            >
              <span class="w-3 shrink-0 text-slate-500">{$expandedStore[g.id] !== false ? "▾" : "▸"}</span>
              <span class="min-w-0 flex-1 truncate font-semibold text-slate-300">{g.label}</span>
              <span class="shrink-0 text-[10px] text-slate-600">{g.panes.length}</span>
            </button>
            <button
              class="shrink-0 rounded px-1 text-sm text-slate-500 hover:bg-slate-700 hover:text-slate-200 disabled:opacity-40"
              disabled={$connState !== "connected" || creatingPane === g.id}
              title="在此 workspace 新建终端"
              on:click={() => createPane(g.id)}
            >
              {creatingPane === g.id ? "…" : "+"}
            </button>
          </div>
          <!-- panes -->
          {#if $expandedStore[g.id] !== false}
            <div class="ml-3 flex flex-col gap-0.5 border-l border-slate-800 pl-1">
              {#each g.panes as p (p.sid)}
                <button
                  class="flex w-full items-center gap-1.5 rounded-md px-1.5 py-1 text-left text-sm {$focusPrefix ===
                  p.sid
                    ? "bg-blue-500/10 text-slate-100"
                    : $windows.has(p.sid)
                      ? "text-slate-300 hover:bg-slate-800"
                      : "text-slate-400 hover:bg-slate-800"}"
                  on:click={() => openWindow(p.sid)}
                >
                  <span class="h-1.5 w-1.5 shrink-0 rounded-full {stateColor(p.agent_status)}"></span>
                  <span class="min-w-0 flex-1 truncate">{p.title || p.pane_id}</span>
                  {#if $windows.has(p.sid)}
                    <span class="shrink-0 text-[9px] text-slate-600">画布</span>
                  {/if}
                  {#if p.state}
                    <span
                      class="shrink-0 text-[9px] {p.state === 'waiting'
                        ? 'text-yellow-400'
                        : 'text-green-400'}"
                    >
                      {p.state}
                    </span>
                  {/if}
                  <span
                    class="shrink-0 rounded px-0.5 text-[10px] text-slate-600 hover:bg-red-900/60 hover:text-red-300"
                    title="关闭此 pane"
                    role="button"
                    tabindex="0"
                    on:click|stopPropagation={(e) => killPane(e, p.pane_id)}
                    on:keydown={(e) => e.key === 'Enter' && killPane(e, p.pane_id)}
                  >
                    ×
                  </span>
                </button>
              {:else}
                <p class="px-1.5 py-0.5 text-[10px] text-slate-600">(空 workspace)</p>
              {/each}
            </div>
          {/if}
        </div>
      {:else}
        {#if $connState !== "connected"}
          <p class="px-2 py-4 text-center text-xs text-slate-500">未连接网关——点顶部状态栏配置</p>
        {/if}
      {/each}

      <!-- orphan panes (workspace list not arrived yet) -->
      {#if orphans.length > 0}
        <div class="mb-0.5 mt-1">
          <div class="flex w-full items-center gap-1 rounded-lg px-1 py-1 text-xs text-slate-400">
            <span class="w-3 shrink-0 text-slate-500">▾</span>
            <span class="min-w-0 flex-1 font-semibold text-slate-300">其他</span>
            <span class="shrink-0 text-[10px] text-slate-600">{orphans.length}</span>
          </div>
          <div class="ml-3 flex flex-col gap-0.5 border-l border-slate-800 pl-1">
            {#each orphans as p (p.sid)}
              <button
                class="flex w-full items-center gap-1.5 rounded-md px-1.5 py-1 text-left text-sm text-slate-400 hover:bg-slate-800"
                on:click={() => openWindow(p.sid)}
              >
                <span class="h-1.5 w-1.5 shrink-0 rounded-full {stateColor(p.agent_status)}"></span>
                <span class="min-w-0 flex-1 truncate">{p.title || p.pane_id}</span>
              </button>
            {/each}
          </div>
        </div>
      {/if}

      <!-- new workspace -->
      <button
        class="mt-1 flex w-full items-center justify-center gap-1 rounded-lg border border-dashed border-slate-600 px-2 py-1.5 text-sm text-slate-300 hover:bg-slate-800 disabled:opacity-40"
        disabled={$connState !== "connected" || creatingWorkspace}
        title="新建 workspace(带一个终端)"
        on:click={createWorkspace}
      >
        <span class="text-base leading-none">+</span>
        {creatingWorkspace ? "创建中…" : "新建 workspace"}
      </button>
    </div>
  </div>
{/if}
