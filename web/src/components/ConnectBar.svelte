<script lang="ts">
  import { onMount } from "svelte";
  import {
    connection,
    loadGatewayUrl,
    validateGatewayUrl,
    mixedContentWarning,
  } from "../lib/connection";
  import { drawerOpen, filesOpen } from "../lib/canvas";
  import { appearance, resolveBgUrl, type Appearance } from "../lib/appearance";
  import { TERM_PRESETS } from "../lib/theme";

  const KEYBAR_MODES: { val: Appearance["keybar"]; label: string }[] = [
    { val: "float", label: "悬浮" },
    { val: "top", label: "置顶" },
    { val: "hidden", label: "隐藏" },
  ];

  let url = "";
  let showSettings = false;
  let headerEl: HTMLElement;

  const connState = connection.state;
  const connError = connection.error;

  onMount(() => {
    url = loadGatewayUrl();
    if (url) doConnect();

    // Click anywhere outside the header closes the settings panel.
    const onDocClick = (e: MouseEvent) => {
      if (!showSettings) return;
      if (headerEl && !headerEl.contains(e.target as Node)) showSettings = false;
    };
    document.addEventListener("click", onDocClick, true);
    return () => document.removeEventListener("click", onDocClick, true);
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

<header
  class="relative z-30 flex items-center gap-2 border-b border-slate-800 bg-[#16161e] px-3 py-2"
  bind:this={headerEl}
>
  <!-- Hamburger menu: toggles the settings panel. -->
  <button
    class="flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-slate-300 hover:bg-slate-800 {showSettings
      ? 'bg-slate-800'
      : ''}"
    title="菜单"
    on:click={() => (showSettings = !showSettings)}
  >
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
    >
      <path d="M3 6h18M3 12h18M3 18h18" />
    </svg>
  </button>

  <button
    class="ml-1 rounded-md px-2 py-0.5 text-xs font-medium {badge[state][1]}"
    on:click={() => (showSettings = !showSettings)}
    title="连接状态:{state === 'connected' ? '点击配置' : '点击配置连接'} · {url}"
  >
    {badge[state][0]}
  </button>

  <div class="ml-auto flex items-center gap-2">
    <button
      class="rounded-md {$filesOpen
        ? 'bg-blue-600'
        : 'bg-slate-800'} px-2.5 py-1 text-xs font-medium text-slate-200 hover:bg-slate-700"
      on:click={() => {
        filesOpen.update((v) => !v);
        drawerOpen.set(false);
        showSettings = false;
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
        showSettings = false;
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

        <!-- appearance -->
        <div class="mt-2 border-t border-slate-700 pt-2">
          <div class="mb-1.5 text-xs font-semibold text-slate-400">外观</div>
          <label class="mb-1.5 flex items-center gap-2 text-xs text-slate-400">
            终端主题
            <select
              class="flex-1 rounded-md border border-slate-700 bg-slate-900 px-2 py-1 text-sm text-slate-100 outline-none focus:border-blue-500"
              value={$appearance.theme}
              on:change={(e) => appearance.set({ ...$appearance, theme: e.currentTarget.value })}
            >
              {#each Object.entries(TERM_PRESETS) as [key, p] (key)}
                <option value={key}>{p.label}</option>
              {/each}
            </select>
          </label>
          <label class="mb-1.5 flex items-center gap-2 text-xs text-slate-400">
            画布背景
            <input
              class="min-w-0 flex-1 rounded-md border border-slate-700 bg-slate-900 px-2 py-1 text-xs text-slate-100 outline-none focus:border-blue-500"
              placeholder="文件路径或图片 URL(留空 = 无)"
              bind:value={$appearance.canvasBg}
              spellcheck="false"
            />
          </label>
          <label class="flex items-center gap-2 text-xs text-slate-400">
            终端背景
            <input
              class="min-w-0 flex-1 rounded-md border border-slate-700 bg-slate-900 px-2 py-1 text-xs text-slate-100 outline-none focus:border-blue-500"
              placeholder="文件路径或图片 URL(留空 = 无)"
              bind:value={$appearance.termBg}
              spellcheck="false"
            />
          </label>
          <div class="flex items-center gap-2 text-xs text-slate-400">
            辅助键
            <div class="flex flex-1 overflow-hidden rounded-md border border-slate-700">
              {#each KEYBAR_MODES as m (m.val)}
                <button
                  class="flex-1 py-1 text-xs {$appearance.keybar === m.val
                    ? 'bg-blue-600 text-white'
                    : 'bg-slate-900 text-slate-300 hover:bg-slate-800'}"
                  on:click={() => appearance.set({ ...$appearance, keybar: m.val })}
                >
                  {m.label}
                </button>
              {/each}
            </div>
          </div>
          <p class="text-[10px] text-slate-600">
            相对路径走网关文件 API;图片窗口标题栏也有「画布」「终端」快捷设置按钮。
          </p>
        </div>
      </div>
    </div>
  {/if}
</header>
