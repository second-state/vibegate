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
  import { t, tNow } from "../lib/i18n";
  import { copyTextToClipboard, showCopiedToast, showToast } from "../lib/session";
  import { openImageBlob } from "../lib/canvas";
  import WebMWriter from "../lib/vendor/webmwriter/index.js";
  import { toPng, toCanvas } from "html-to-image";
  import { get } from "svelte/store";

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
  /** Theme-stripped source computed on mount — reused by video export. */
  let exportSrc: Parameters<typeof createPlayer>[0] | null = null;

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

  /** Generate a webm video from the player's own rendering: an offscreen
   *  player instance is stepped through the recording's frame times (seek),
   *  a MutationObserver waits for each resulting DOM change, and
   *  html-to-image rasterizes the frame. Slow but faithful — the video is
   *  built from exactly what the visible player shows. */
  let generating = false;
  let genPct = 0; // bar width only — the label shows events done/total
  let genDone = 0;
  let genTotal = 0;
  async function genVideo() {
    closeCtxMenu();
    if (generating) return;
    generating = true;
    genPct = 0;
    genDone = 0;
    genTotal = 0;
    let holder: HTMLDivElement | null = null;
    let obs: MutationObserver | null = null;
    let offscreen: ReturnType<typeof createPlayer> | null = null;
    try {
      const resp = await fetch(url);
      const lines = (await resp.text()).split("\n").filter((l) => l.trim());
      const header = JSON.parse(lines[0]);
      if (header.version !== 3) throw new Error("only asciicast v3 is supported");
      // frame start times (absolute seconds, deduped at 40ms resolution)
      const frameTimes: number[] = [0];
      let elapsed = 0;
      for (const line of lines.slice(1)) {
        const [dt] = JSON.parse(line);
        elapsed += Number(dt) || 0;
        if (elapsed - frameTimes[frameTimes.length - 1] >= 0.04) frameTimes.push(elapsed);
      }
      const total = Number(header.duration) || elapsed + 1;
      genTotal = frameTimes.length;

      // offscreen player: same theme-stripped source as the visible one
      const holderW = containerEl.clientWidth || 720;
      const holderH = containerEl.clientHeight || 480;
      holder = document.createElement("div");
      holder.className = "vg-export-stage";
      holder.style.cssText = `position:fixed;left:-99999px;top:0;width:${holderW}px;height:${holderH}px;overflow:hidden;background:${getAppearance().termBg ? "#000000" : "#16161e"};`;
      const stage = document.createElement("div");
      stage.style.cssText = "position:absolute;inset:0;";
      for (const v of ["--term-color-background", "--term-color-foreground", "--term-font-family"]) {
        stage.style.setProperty(v, containerEl.style.getPropertyValue(v));
      }
      if (getAppearance().termBg) {
        const o = document.createElement("div");
        o.style.cssText = `position:absolute;inset:0;background-image:url("${resolveBgUrl(getAppearance().termBg)}");background-size:cover;background-position:center;opacity:${Math.min(100, Math.max(0, getAppearance().termBgOpacity ?? 100)) / 100};`;
        stage.appendChild(o);
      }
      // export-only chrome fixes: hide the big ▶/loader overlays (the
      // offscreen player never "plays"), and screen-blend the player over
      // the bg image so the artwork shows through (same as the live window)
      const exportStyle = document.createElement("style");
      exportStyle.textContent = `
        .vg-export-stage .ap-overlay { display: none !important; }
        ${
          getAppearance().termBg
            ? `.vg-export-stage .ap-player,
               .vg-export-stage .ap-term,
               .vg-export-stage canvas { mix-blend-mode: screen; }`
            : ""
        }
      `;
      holder.appendChild(exportStyle);
      holder.appendChild(stage);
      document.body.appendChild(holder);

      const raf = () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
      // resolves on the next DOM mutation inside the offscreen player
      let notify: (() => void) | null = null;
      obs = new MutationObserver(() => notify?.());
      obs.observe(stage, { subtree: true, childList: true, attributes: true, characterData: true });
      const waitChange = (ms: number) =>
        new Promise<void>((r) => {
          let done = false;
          notify = () => {
            if (!done) {
              done = true;
              r();
            }
          };
          setTimeout(() => {
            done = true;
            r();
          }, ms);
        });

      const src = exportSrc ?? url; // theme-stripped source from onMount
      offscreen = createPlayer(src, stage, {
        autoPlay: false,
        preload: true,
        controls: false,
        fit: "both",
        logger: {
          debug: (m) => ((window as any).__aplog = (window as any).__aplog || []).push(["debug", m]),
          info: (m) => ((window as any).__aplog = (window as any).__aplog || []).push(["info", m]),
          warn: (m) => ((window as any).__aplog = (window as any).__aplog || []).push(["warn", m]),
          error: (m) => ((window as any).__aplog = (window as any).__aplog || []).push(["error", m]),
        },
      });
      await waitChange(2000); // initial render

      // The player letterboxes the terminal inside the stage (.ap-term only
      // covers the terminal region — the rest shows the raw bg overlay).
      // Measure the term rect EVERY frame: the player can resize after the
      // initial render (real font settles on first seek), so a one-shot
      // measurement goes stale and leaves bands / cuts content.
      const stageW = stage.clientWidth || 1;

      const writer = new WebMWriter({ quality: 0.95, frameRate: 30 });
      for (let k = 0; k < frameTimes.length; k++) {
        (window as any).__aplog.push(["seek", frameTimes[k]]);
        await offscreen.seek(frameTimes[k]);
        await waitChange(800); // frame DOM settled
        await raf();
        const frameCanvas = await toCanvas(stage, {
          pixelRatio: Math.min(window.devicePixelRatio || 1, 2),
        });
        let out = frameCanvas;
        const termEl = stage.querySelector(".ap-term") as HTMLElement | null;
        if (termEl) {
          const sr = stage.getBoundingClientRect();
          const tr = termEl.getBoundingClientRect();
          const crop = {
            x: Math.max(0, Math.round(tr.left - sr.left)),
            y: Math.max(0, Math.round(tr.top - sr.top)),
            w: Math.max(1, Math.round(tr.width)),
            h: Math.max(1, Math.round(tr.height)),
          };
          const s = frameCanvas.width / stageW; // raster scale (incl. pixelRatio)
          const c = document.createElement("canvas");
          c.width = Math.max(2, Math.round(crop.w * s) & ~1); // even sizes keep
          c.height = Math.max(2, Math.round(crop.h * s) & ~1); // encoders happy
          c.getContext("2d")!.drawImage(
            frameCanvas,
            Math.round(crop.x * s),
            Math.round(crop.y * s),
            Math.round(crop.w * s),
            Math.round(crop.h * s),
            0,
            0,
            c.width,
            c.height,
          );
          out = c;
        }
        const nextT = k + 1 < frameTimes.length ? frameTimes[k + 1] : total + 1;
        writer.addFrame(out, Math.max(1, Math.round((nextT - frameTimes[k]) * 1000)));
        genDone = k + 1;
        genPct = Math.round((genDone / frameTimes.length) * 100);
      }
      (window as any).__aplog.push(["completing"]);
      const blob = await writer.complete();
      (window as any).__aplog.push(["download"]);

      const dlName = name.replace(/\.cast$/i, "") + ".webm";
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = dlName;
      a.click();
      setTimeout(() => URL.revokeObjectURL(a.href), 5000);
      (window as any).__aplog.push(["pre-toast"]);
      showToast(get(t)("exportSaved", { name: dlName }));
      (window as any).__aplog.push(["post-toast"]);
    } catch (e) {
      (window as any).__aplog.push(["catch", String(e), (e as Error).stack?.slice(0, 300)]);
      showCopiedToast(false, String(e));
    } finally {
      obs?.disconnect();
      try { offscreen?.dispose(); } catch { /* dispose of a never-played offscreen player can throw */ }
      holder?.remove();
      generating = false;
    }
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
    exportSrc = src;

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
      {#if generating}
        <!-- video generation progress overlay -->
        <div
          class="pointer-events-none absolute inset-0 z-20 flex flex-col items-center justify-center gap-2 bg-[#16161e]/85"
        >
          <p class="text-xs text-slate-300">{$t("genVideo")} — {genDone}/{genTotal}</p>
          <div class="h-1.5 w-3/5 overflow-hidden rounded bg-slate-800">
            <div
              class="h-full bg-blue-500 transition-all duration-200"
              style:width="{genPct}%"
            ></div>
          </div>
        </div>
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
    <button
      class="block w-full px-3 py-1.5 text-left text-xs {generating
        ? 'text-blue-400'
        : 'text-slate-200 hover:bg-slate-700'}"
      disabled={generating}
      on:mousedown|preventDefault={genVideo}
    >
      {generating ? `${genDone}/${genTotal}` : $t("genVideo")}
    </button>
  </div>
{/if}
