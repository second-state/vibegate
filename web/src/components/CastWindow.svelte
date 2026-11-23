<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { create as createPlayer } from "asciinema-player";
  import "asciinema-player/dist/bundle/asciinema-player.css";
  import {
    windows,
    focusPrefix,
    moveWindow,
    closeWindow,
    canvasOnly,
  } from "../lib/canvas";
  import { nativePointerDown, stopNative } from "../lib/actions";
  import { connection } from "../lib/connection";
  import { getAppearance, appearance, resolveBgUrl } from "../lib/appearance";
  import { TERM_PRESETS } from "../lib/theme";
  import { t } from "../lib/i18n";
  import { copyTextToClipboard, showCopiedToast } from "../lib/session";
  import { openImageBlob } from "../lib/canvas";
  import { toPng } from "html-to-image";

  export let path: string; // full file path (without the "cast:" key prefix)
  export let zoom = 1;

  $: rect = $windows.get(key());
  $: focused = $focusPrefix === key();
  $: name = path.split("/").pop() || path;
  $: url = `${connection.apiBase}/api/file?path=${encodeURIComponent(path)}&inline=1`;

  function key() {
    return "cast:" + path;
  }

  let containerEl: HTMLElement;
  let player: ReturnType<typeof createPlayer> | null = null;

  /** Screenshot the player. The player renders text as DOM (pre/span),
   *  NOT on a canvas — so this needs a DOM rasterizer (html-to-image),
   *  which also captures the bg image overlay in one shot. */
  async function shotCast() {
    closeCtxMenu();
    if (!containerEl) return;
    try {
      const dataUrl = await toPng(containerEl, { pixelRatio: Math.min(window.devicePixelRatio || 1, 3) });
      const ts = new Date();
      const p2 = (n: number) => String(n).padStart(2, "0");
      const namePng = name.replace(/\.cast$/i, "") + `-${p2(ts.getHours())}${p2(ts.getMinutes())}${p2(ts.getSeconds())}.png`;
      openImageBlob(namePng, dataUrl, Math.round(containerEl.clientWidth) + 16, Math.round(containerEl.clientHeight) + 16);
    } catch (e) {
      showCopiedToast(false, String(e));
    }
  }

  // --- right-click menu (screenshot etc.) ---
  let ctxMenu: { x: number; y: number } | null = null;
  function openCtxMenu(x: number, y: number) {
    ctxMenu = {
      x: Math.min(x, window.innerWidth - 170),
      y: Math.min(y, window.innerHeight - 160),
    };
    document.addEventListener("mousedown", closeCtxOnOutside, true);
    document.addEventListener("keydown", closeCtxOnEsc, true);
  }
  function closeCtxMenu() {
    ctxMenu = null;
    document.removeEventListener("mousedown", closeCtxOnOutside, true);
    document.removeEventListener("keydown", closeCtxOnEsc, true);
  }
  function closeCtxOnOutside(e: MouseEvent) {
    const t = e.target as HTMLElement;
    if (!t.closest("[data-ctx-menu]")) closeCtxMenu();
  }
  function closeCtxOnEsc(e: KeyboardEvent) {
    if (e.key === "Escape") closeCtxMenu();
  }
  /** Render the menu at body level: fixed positioning inside the transformed
   *  world container would be scaled/offset away from the cursor. */
  function portalToBody(node: HTMLElement) {
    document.body.appendChild(node);
  }
  function ctxOpenAt(e: MouseEvent) {
    e.preventDefault();
    openCtxMenu(e.clientX, e.clientY);
  }

  onMount(async () => {
    const a = getAppearance();
    const preset = TERM_PRESETS[a.theme] ?? TERM_PRESETS["tokyo-night"];
    const theme = preset.theme;
    // Theme the player via its CSS custom properties. The cast file's own
    // header theme is stripped (see below) so the page settings win.
    containerEl.style.setProperty("--term-color-background", theme.background ?? "#1a1b26");
    containerEl.style.setProperty("--term-color-foreground", theme.foreground ?? "#c0caf5");
    containerEl.style.setProperty("--term-font-family", "ui-monospace, SFMono-Regular, Menlo, monospace");

    // Strip the recording's embedded theme from the v3 header and replace
    // it with the page theme — otherwise the player paints with whatever
    // terminal theme the author had (their "green" may be our orange).
    // Falls back to the URL if the file can't be read/rewritten.
    let src: Parameters<typeof createPlayer>[0] = url;
    try {
      const resp = await fetch(url);
      const lines = (await resp.text()).split("\n");
      const header = JSON.parse(lines[0]);
      if (header?.version === 3 && header.term) {
        const pal = [
          theme.black,
          theme.red,
          theme.green,
          theme.yellow,
          theme.blue,
          theme.magenta,
          theme.cyan,
          theme.white,
          theme.brightBlack,
          theme.brightRed,
          theme.brightGreen,
          theme.brightYellow,
          theme.brightBlue,
          theme.brightMagenta,
          theme.brightCyan,
          theme.brightWhite,
        ]
          .map((c) => c ?? "#000000")
          .join(":");
        header.term.theme = {
          fg: theme.foreground ?? "#c0caf5",
          bg: theme.background ?? "#1a1b26",
          palette: pal,
        };
        lines[0] = JSON.stringify(header);
        src = { data: lines.join("\n"), parser: "asciicast" };
      }
    } catch {
      /* keep the URL source */
    }

    player = createPlayer(src, containerEl, {
      autoPlay: true,
      controls: "auto", // show the play bar on hover
      idleTimeLimit: 2, // clamp idle gaps so recordings stay watchable
      fit: "both",
      logger: {
        debug: (m) => ((window as any).__aplog = (window as any).__aplog || []).push(["debug", m]),
        info: (m) => ((window as any).__aplog = (window as any).__aplog || []).push(["info", m]),
        warn: (m) => ((window as any).__aplog = (window as any).__aplog || []).push(["warn", m]),
        error: (m) => ((window as any).__aplog = (window as any).__aplog || []).push(["error", m]),
      },
    });
    player.addEventListener("ended", () => endedHandler?.());
  });

  onDestroy(() => {
    player?.dispose();
    player = null;
  });

  // --- header drag ---
  let drag: { px: number; py: number; x: number; y: number } | null = null;
  function headerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    if (!rect) return;
    focusPrefix.set(key());
    drag = { px: e.clientX, py: e.clientY, x: rect.x, y: rect.y };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function headerMove(e: PointerEvent) {
    if (!drag || !rect) return;
    moveWindow(key(), {
      ...rect,
      x: Math.round(drag.x + (e.clientX - drag.px) / zoom),
      y: Math.round(drag.y + (e.clientY - drag.py) / zoom),
    });
  }
  function headerUp() {
    drag = null;
  }
</script>

{#if rect}
  <div
    class="absolute flex flex-col overflow-hidden rounded-lg border shadow-2xl {focused
      ? 'border-slate-500'
      : 'border-slate-800 opacity-90'}"
    class:pointer-events-none={$canvasOnly}
    style:left="{rect.x}px"
    style:top="{rect.y}px"
    style:width="{rect.w}px"
    style:height="{rect.h}px"
    use:nativePointerDown={() => focusPrefix.set(key())}
    use:stopNative={["dblclick"]}
    on:contextmenu={ctxOpenAt}
    role="presentation"
    data-focused-window={focused ? "1" : undefined}
    data-chrome={getAppearance().chrome}
    data-window="1"
  >
    <div
      class="win-head pointer-events-auto flex h-8 shrink-0 cursor-grab select-none items-center gap-2 border-b border-slate-800 bg-[#1f2233] px-2 active:cursor-grabbing"
      role="presentation"
      use:nativePointerDown={headerDown}
      on:pointermove={headerMove}
      on:pointerup={headerUp}
      on:pointercancel={headerUp}
    >
      <span class="shrink-0 text-slate-500">📼</span>
      <span class="min-w-0 flex-1 truncate text-xs font-medium text-slate-300" title={path}>
        {name}
      </span>
      <button
        class="shrink-0 rounded px-1 text-[10px] text-slate-500 hover:bg-slate-700 hover:text-slate-200"
        title={$t("copyPath")}
        use:stopNative={["pointerdown"]}
        on:mousedown={(e) =>
          e.button === 0 &&
          copyTextToClipboard(path)
            .then(() => showCopiedToast(true, ""))
            .catch((er) => showCopiedToast(false, String(er)))}
      >
        ⧉
      </button>
      <a
        class="shrink-0 rounded px-1 text-xs text-slate-500 hover:text-slate-200"
        title={$t("download")}
        href={connection.apiBase + "/api/file?path=" + encodeURIComponent(path)}
        download={name}
        use:stopNative={["pointerdown"]}
      >
        ⇩
      </a>
      <button
        class="shrink-0 rounded px-1.5 text-sm text-slate-500 hover:text-red-400"
        title={$t("closeWin")}
        use:stopNative={["pointerdown", "dblclick"]}
        on:mousedown={(e) => e.button === 0 && closeWindow(key())}
      >
        ×
      </button>
    </div>

    <div
      class="relative min-h-0 flex-1 overflow-hidden bg-[#16161e]"
      bind:this={containerEl}
    >
      {#if getAppearance().termBg}
        <!-- bg image goes ON TOP of the player at reduced opacity: the
             player's canvas paints an opaque theme background, so an
             image behind it could never show through. pointer-events-none
             keeps the player interactive. -->
        <div
          class="pointer-events-none absolute inset-0 z-10 bg-cover bg-center"
          style:background-image={`url("${resolveBgUrl(getAppearance().termBg)}")`}
          style:opacity={Math.min(100, Math.max(0, getAppearance().termBgOpacity ?? 100)) / 100}
        ></div>
      {/if}
    </div>
  </div>
{/if}

{#if ctxMenu}
  <div
    data-ctx-menu
    use:portalToBody
    class="fixed z-[60] min-w-[170px] overflow-hidden rounded-md border border-slate-700 bg-[#1f2233] py-1 shadow-2xl"
    style:left="{ctxMenu.x}px"
    style:top="{ctxMenu.y}px"
  >
    <button
      class="block w-full px-3 py-1.5 text-left text-xs text-slate-200 hover:bg-slate-700"
      on:mousedown|preventDefault={shotCast}
    >
      {$t("ctxScreenshot")}
    </button>
  </div>
{/if}
