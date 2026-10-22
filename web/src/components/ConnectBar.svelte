<script lang="ts">
  import { onMount } from "svelte";
  import {
    connection,
    loadGatewayUrl,
    validateGatewayUrl,
    mixedContentWarning,
  } from "../lib/connection";
  import { drawerOpen, filesOpen } from "../lib/canvas";

  let url = "";
  let showSettings = false;

  const connState = connection.state;
  const connError = connection.error;

  onMount(() => {
    url = loadGatewayUrl();
    if (url) doConnect();
  });

  $: state = $connState;
  $: err = $connError;
  $: mixedWarn = mixedContentWarning(url);

  const badge = {
    idle: ["未连接", "bg-slate-700 text-slate-300"],
    connecting: ["连接中…", "bg-yellow-800 text-yellow-200"],
    connected: ["已连接", "bg-green-900 text-green-300"],
    reconnecting: ["重连中…", "bg-orange-900 text-orange-200"],
    error: ["错误", "bg-red-900 text-red-300"],
  } as const;

  function doConnect() {
    const verr = validateGatewayUrl(url.trim());
    if (verr) {
      connection.error.set(verr);
      connection.state.set("error");
      return;
    }
    connection.connect(url.trim());
  }

  function doDisconnect() {
    connection.disconnect();
  }

  /** Enter in the URL field applies the settings immediately. */
  function fieldKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      showSettings = false;
      doConnect();
    }
  }
</script>

<header class="relative z-20 flex items-center gap-2 border-b border-slate-800 bg-[#16161e] px-3 py-2">
  <div class="flex items-baseline gap-2">
    <span class="text-sm font-bold tracking-wide text-slate-100">vibeview</span>
    <span class="hidden text-xs text-slate-500 sm:inline">vibegate 终端查看端</span>
  </div>

  <button
    class="ml-2 rounded-md px-2 py-0.5 text-xs font-medium {badge[state][1]}"
    on:click={() => (showSettings = !showSettings)}
    title="点击{state === 'connected' ? '查看' : '配置'}连接"
  >
    {badge[state][0]}
  </button>

  <!-- Gateway address: click to edit (desktop). -->
  <button
    class="hidden max-w-[24rem] items-center gap-1 truncate text-left text-xs text-slate-500 hover:text-slate-300 md:flex"
    on:click={() => (showSettings = true)}
    title="编辑 vibegate 地址"
  >
    <span class="truncate">{url || "未配置地址"}</span>
    <span class="shrink-0 text-slate-600">✎</span>
  </button>

  <div class="ml-auto flex items-center gap-2">
    <!-- Edit connection (mobile: the address button above is hidden). -->
    <button
      class="rounded-md bg-slate-800 px-2 py-1 text-xs text-slate-300 hover:bg-slate-700 md:hidden"
      on:click={() => (showSettings = true)}
      title="编辑 vibegate 地址"
    >
      ✎
    </button>
    <button
      class="rounded-md {$filesOpen
        ? 'bg-blue-600'
        : 'bg-slate-800'} px-2.5 py-1 text-xs font-medium text-slate-200 hover:bg-slate-700"
      on:click={() => {
        filesOpen.update((v) => !v);
        drawerOpen.set(false);
      }}
    >
      文件
    </button>
    <button
      class="rounded-md {$drawerOpen
        ? 'bg-blue-600'
        : 'bg-slate-800'} px-2.5 py-1 text-xs font-medium text-slate-200 hover:bg-slate-700"
      on:click={() => {
        drawerOpen.update((v) => !v);
        filesOpen.set(false);
      }}
    >
      会话
    </button>
  </div>

  {#if showSettings}
    <div class="absolute inset-x-2 top-full mt-1 rounded-lg border border-slate-700 bg-[#1a1b26] p-3 shadow-xl">
      <div class="flex flex-col gap-2">
        <label class="flex flex-col gap-1 text-xs text-slate-400">
          vibegate 网关地址
          <input
            class="rounded-md border border-slate-700 bg-slate-900 px-2 py-1.5 text-sm text-slate-100 outline-none focus:border-blue-500"
            style="font-size: 16px"
            bind:value={url}
            placeholder="ws://192.168.1.10:8480/ws"
            spellcheck="false"
            on:keydown={fieldKeydown}
          />
        </label>
        <p class="text-xs text-slate-500">
          即 vibegate 服务端的 <code class="text-slate-400">/ws</code> 地址;https 页面请用 wss://。
        </p>
        {#if mixedWarn}
          <p class="text-xs text-yellow-400">{mixedWarn}</p>
        {/if}
        {#if err}
          <p class="text-xs text-red-400">{err}</p>
        {/if}
        <div class="flex gap-2">
          <button
            class="rounded-md bg-blue-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-blue-500"
            on:click={() => {
              showSettings = false;
              doConnect();
            }}
          >
            连接
          </button>
          <button
            class="rounded-md bg-slate-800 px-3 py-1.5 text-sm text-slate-300 hover:bg-slate-700"
            on:click={doDisconnect}
          >
            断开
          </button>
        </div>
      </div>
    </div>
  {/if}
</header>
