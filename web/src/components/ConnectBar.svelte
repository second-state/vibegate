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
  import { t, lang, type Lang } from "../lib/i18n";

  const KEYBAR_MODES: {
    val: Appearance["keybar"];
    label: "keybarFloat" | "keybarTop" | "keybarHidden";
  }[] = [
    { val: "float", label: "keybarFloat" },
    { val: "top", label: "keybarTop" },
    { val: "hidden", label: "keybarHidden" },
  ];
  const LANGS: { val: Lang; label: string }[] = [
    { val: "zh", label: "中文" },
    { val: "en", label: "English" },
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

  const badgeCls = {
    idle: "bg-slate-700 text-slate-300",
    connecting: "bg-yellow-800 text-yellow-200",
    connected: "bg-green-900 text-green-300",
    reconnecting: "bg-orange-900 text-orange-200",
    error: "bg-red-900 text-red-300",
  } as const;
  const badgeKey = {
    idle: "stateIdle",
    connecting: "stateConnecting",
    connected: "stateConnected",
    reconnecting: "stateReconnecting",
    error: "stateError",
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
    title={$t("menu")}
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
    class="ml-1 rounded-md px-2 py-0.5 text-xs font-medium {badgeCls[state]}"
    on:click={() => (showSettings = !showSettings)}
    title="{$t("badgeTitle", { state: $t(badgeKey[state]) })} · {url}"
  >
    {$t(badgeKey[state])}
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
      {$t("files")}
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
      {$t("sessions")}
    </button>
  </div>

  {#if showSettings}
    <div class="absolute left-2 top-full mt-1 w-[calc(100vw-1rem)] max-w-md rounded-lg border border-slate-700 bg-[#1a1b26] p-3 shadow-xl">
      <div class="flex flex-col gap-2">
        <label class="flex flex-col gap-1 text-xs text-slate-400">
          {$t("gatewayUrl")}
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
          {$t("gatewayHint")}
        </p>
        {#if mixedWarn}
          <p class="text-xs text-yellow-400">{$t("mixedContent")}</p>
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
            {$t("connect")}
          </button>
          <button
            class="rounded-md bg-slate-800 px-3 py-1.5 text-sm text-slate-300 hover:bg-slate-700"
            on:click={doDisconnect}
          >
            {$t("disconnect")}
          </button>
        </div>

        <!-- appearance: label on one line, control on the next -->
        <div class="mt-2 border-t border-slate-700 pt-2">
          <div class="mb-1.5 text-xs font-semibold text-slate-400">{$t("appearance")}</div>
          <label class="mb-1.5 block text-xs text-slate-400">
            <div class="mb-1">{$t("terminalTheme")}</div>
            <select
              class="w-full rounded-md border border-slate-700 bg-slate-900 px-2 py-1 text-sm text-slate-100 outline-none focus:border-blue-500"
              value={$appearance.theme}
              on:change={(e) => appearance.set({ ...$appearance, theme: e.currentTarget.value })}
            >
              {#each Object.entries(TERM_PRESETS) as [key, p] (key)}
                <option value={key}>{p.label}</option>
              {/each}
            </select>
          </label>
          <label class="mb-1.5 block text-xs text-slate-400">
            <div class="mb-1">{$t("canvasBgLabel")}</div>
            <input
              class="w-full rounded-md border border-slate-700 bg-slate-900 px-2 py-1 text-xs text-slate-100 outline-none focus:border-blue-500"
              placeholder={$t("bgPlaceholder")}
              bind:value={$appearance.canvasBg}
              spellcheck="false"
            />
          </label>
          <label class="block text-xs text-slate-400">
            <div class="mb-1">{$t("terminalBgLabel")}</div>
            <input
              class="w-full rounded-md border border-slate-700 bg-slate-900 px-2 py-1 text-xs text-slate-100 outline-none focus:border-blue-500"
              placeholder={$t("bgPlaceholder")}
              bind:value={$appearance.termBg}
              spellcheck="false"
            />
          </label>
          {#if $appearance.termBg}
            <div class="mt-1.5 text-xs text-slate-400">
              <div class="mb-1">{$t("bgOpacity")}</div>
              <div class="flex items-center gap-2">
                <input
                  type="range"
                  min="0"
                  max="100"
                  step="1"
                  class="min-w-0 flex-1 accent-blue-500"
                  bind:value={$appearance.termBgOpacity}
                />
                <input
                  type="number"
                  min="0"
                  max="100"
                  class="w-14 shrink-0 rounded-md border border-slate-700 bg-slate-900 px-1.5 py-1 text-right text-xs text-slate-100 outline-none focus:border-blue-500"
                  bind:value={$appearance.termBgOpacity}
                />
                <span class="shrink-0">%</span>
              </div>
            </div>
          {/if}
          <div class="mt-1.5 mb-1.5 text-xs text-slate-400">
            <div class="mb-1">{$t("keybarLabel")}</div>
            <div class="flex overflow-hidden rounded-md border border-slate-700">
              {#each KEYBAR_MODES as m (m.val)}
                <button
                  class="flex-1 py-1 text-xs {$appearance.keybar === m.val
                    ? 'bg-blue-600 text-white'
                    : 'bg-slate-900 text-slate-300 hover:bg-slate-800'}"
                  on:click={() => appearance.set({ ...$appearance, keybar: m.val })}
                >
                  {$t(m.label)}
                </button>
              {/each}
            </div>
          </div>
          <div class="text-xs text-slate-400">
            <div class="mb-1">{$t("language")}</div>
            <div class="flex overflow-hidden rounded-md border border-slate-700">
              {#each LANGS as l (l.val)}
                <button
                  class="flex-1 py-1 text-xs {$lang === l.val
                    ? 'bg-blue-600 text-white'
                    : 'bg-slate-900 text-slate-300 hover:bg-slate-800'}"
                  on:click={() => lang.set(l.val)}
                >
                  {l.label}
                </button>
              {/each}
            </div>
          </div>
          <p class="text-[10px] text-slate-600">
            {$t("appearanceNote")}
          </p>
        </div>
      </div>
    </div>
  {/if}
</header>
