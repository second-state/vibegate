<script lang="ts">
  import { onMount } from "svelte";
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  import { windows, focusPrefix, moveWindow, closeWindow, canvasOnly } from "../lib/canvas";
  import { nativePointerDown, stopNative } from "../lib/actions";
  import { connection } from "../lib/connection";
  import { t } from "../lib/i18n";

  export let path: string; // full file path (without the "md:" key prefix)
  export let zoom = 1;

  $: rect = $windows.get(key());
  $: focused = $focusPrefix === key();
  $: name = path.split("/").pop() || path;
  $: url = `${connection.apiBase}/api/file?path=${encodeURIComponent(path)}`;

  function key() {
    return "md:" + path;
  }

  let bodyEl: HTMLElement;
  let loaded = false;
  let error = "";

  marked.setOptions({ gfm: true, breaks: false });

  /** Render the fetched markdown: sanitize, resolve relative images to the
   *  gateway file API, then lazily render mermaid fences into SVG (mermaid
   *  only loads when a diagram is actually present). */
  async function render(md: string) {
    const html = DOMPurify.sanitize(marked.parse(md) as string);
    bodyEl.innerHTML = html;
    // Rewrite relative <img> sources against this file's directory.
    const base = path.includes("/") ? path.slice(0, path.lastIndexOf("/") + 1) : "";
    bodyEl.querySelectorAll("img").forEach((img) => {
      const src = img.getAttribute("src") ?? "";
      if (!src || /^(https?:|data:)/i.test(src)) return;
      const full = base + src.replace(/^\.\//, "");
      img.src = `${connection.apiBase}/api/file?path=${encodeURIComponent(full)}`;
    });
    const blocks = [
      ...bodyEl.querySelectorAll("pre > code.language-mermaid"),
    ];
    if (blocks.length) {
      try {
        const mermaid = (await import("mermaid")).default;
        mermaid.initialize({
          startOnLoad: false,
          theme: "dark",
          securityLevel: "strict",
          // HTML labels live in <foreignObject>, which the SVG sanitize
          // profile strips — use plain <text> so labels survive.
          flowchart: { htmlLabels: false },
          class: { htmlLabels: false },
        });
        await Promise.all(
          blocks.map(async (el, i) => {
            const code = el.textContent ?? "";
            const pre = el.parentElement!;
            try {
              const { svg } = await mermaid.render(`mermaid-${Date.now()}-${i}`, code);
              // No extra DOMPurify pass here: mermaid's strict mode already
              // sanitizes, and the svg profile would strip <foreignObject>
              // node labels. The markdown AROUND the diagrams is what needs
              // the sanitize pass above.
              const wrap = document.createElement("div");
              wrap.innerHTML = svg;
              pre.replaceWith(wrap);
            } catch (e) {
              // Bad diagram: keep the source visible with the error attached.
              pre.classList.add("text-red-300", "whitespace-pre-wrap");
              pre.title = String(e);
            }
          }),
        );
      } catch {
        // mermaid failed to load — code blocks stay as plain code.
      }
    }
  }

  onMount(async () => {
    try {
      const resp = await fetch(url);
      if (!resp.ok) {
        error = `HTTP ${resp.status}`;
        return;
      }
      await render(await resp.text());
      loaded = true;
    } catch (e) {
      error = String(e);
    }
  });

  // --- header drag ---
  let drag: { px: number; py: number; x: number; y: number } | null = null;
  function headerDown(e: PointerEvent) {
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
    role="presentation"
  >
    <div
      class="pointer-events-auto flex h-8 shrink-0 cursor-grab select-none items-center gap-2 border-b border-slate-800 bg-[#1f2233] px-2 active:cursor-grabbing"
      role="presentation"
      use:nativePointerDown={headerDown}
      on:pointermove={headerMove}
      on:pointerup={headerUp}
      on:pointercancel={headerUp}
    >
      <span class="shrink-0 text-slate-500">📝</span>
      <span class="min-w-0 flex-1 truncate text-xs font-medium text-slate-300" title={path}>
        {name}
      </span>
      <a
        class="shrink-0 rounded px-1 text-xs text-slate-500 hover:text-slate-200"
        title={$t("download")}
        href={url}
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

    <div class="min-h-0 flex-1 overflow-auto bg-[#101018]">
      {#if error}
        <p class="p-4 text-xs text-red-400">{error}</p>
      {:else if !loaded}
        <p class="p-4 text-xs text-slate-500">{$t("loading")}</p>
      {/if}
      <div
        bind:this={bodyEl}
        class="prose prose-invert prose-sm max-w-none p-4 prose-pre:bg-slate-900 prose-code:text-sky-300 prose-a:text-sky-300"
      ></div>
    </div>
  </div>
{/if}
