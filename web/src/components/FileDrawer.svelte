<script lang="ts">
  import { filesOpen, drawerOpen, openImageWindow } from "../lib/canvas";
  import { connection } from "../lib/connection";

  type Entry = {
    name: string;
    is_dir: boolean;
    size: number;
    mtime?: number | null;
  };

  const IMAGE_EXT = /\.(png|jpe?g|gif|webp|svg|bmp|avif)$/i;
  const isImage = (name: string) => IMAGE_EXT.test(name);

  let path = ""; // "" = gateway file root
  let entries: Entry[] = [];
  let loading = false;
  let errMsg = "";
  let uploading = 0; // files currently in flight
  let fileInput: HTMLInputElement;

  /** HTTP base for the file API, derived from the gateway WS URL. */
  function apiBase(): string {
    const ws = connection.urlValue || `ws://${window.location.host}/ws`;
    return ws.replace(/^ws/i, "http").replace(/\/ws\/?$/i, "");
  }

  async function load(p = path) {
    loading = true;
    errMsg = "";
    try {
      const resp = await fetch(`${apiBase()}/api/files?path=${encodeURIComponent(p)}`);
      const body = await resp.json();
      if (!resp.ok) {
        errMsg = body.message || body.error || `HTTP ${resp.status}`;
        if (body.type === "file") errMsg = "这是一个文件";
        return;
      }
      if (body.type !== "dir") {
        errMsg = "不是目录";
        return;
      }
      path = body.path;
      entries = body.entries ?? [];
    } catch (e) {
      errMsg = String(e);
    } finally {
      loading = false;
    }
  }

  // Load the root listing the first time the panel opens (not on empty-dir
  // results, which would loop).
  let booted = false;
  $: if (!$filesOpen) booted = false;
  $: if ($filesOpen && !booted) {
    booted = true;
    load(path);
  }

  const enter = (e: Entry) => {
    if (!e.is_dir) return;
    load(joinPath(e.name));
  };

  function joinPath(name: string): string {
    if (!path) return name;
    return path.endsWith("/") ? path + name : `${path}/${name}`;
  }

  function up() {
    if (path === "") return;
    const idx = path.replace(/\/$/, "").lastIndexOf("/");
    load(idx <= 0 ? "" : path.slice(0, idx));
  }

  function fmtSize(n: number): string {
    if (n < 1024) return `${n}B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)}K`;
    if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)}M`;
    return `${(n / 1024 / 1024 / 1024).toFixed(1)}G`;
  }

  function fmtTime(ts?: number | null): string {
    if (!ts) return "";
    const d = new Date(ts * 1000);
    return `${d.getMonth() + 1}/${d.getDate()} ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  }

  function pickUpload() {
    fileInput?.click();
  }

  async function uploadFiles(ev: Event) {
    const input = ev.target as HTMLInputElement;
    const files = Array.from(input.files ?? []);
    input.value = "";
    for (const f of files) {
      uploading += 1;
      try {
        const target = joinPath(f.name);
        const resp = await fetch(
          `${apiBase()}/api/file?path=${encodeURIComponent(target)}`,
          { method: "PUT", body: f },
        );
        if (!resp.ok) {
          const body = await resp.json().catch(() => null);
          errMsg = `上传失败 ${f.name}: ${body?.message ?? resp.status}`;
        }
      } catch (e) {
        errMsg = `上传失败 ${f.name}: ${e}`;
      } finally {
        uploading -= 1;
      }
    }
    await load(path);
  }

  $: if ($filesOpen && !entries.length && !loading && !errMsg) {
    // First open: fetch the root listing once.
    load(path);
  }
</script>

{#if $filesOpen}
  <div
    class="absolute right-2 top-2 z-10 flex max-h-[75vh] w-80 max-w-[calc(100vw-1rem)] flex-col overflow-hidden rounded-xl border border-slate-700 bg-slate-900/95 shadow-2xl backdrop-blur"
  >
    <div class="flex items-center justify-between border-b border-slate-800 px-3 py-2">
      <span class="text-xs font-semibold text-slate-300">文件</span>
      <button
        class="rounded px-1 text-xs text-slate-500 hover:text-slate-200"
        title="刷新"
        on:click={() => load(path)}
      >
        ⟳
      </button>
    </div>

    <!-- path bar -->
    <div class="flex items-center gap-1 border-b border-slate-800 px-2 py-1.5">
      <button
        class="rounded px-1.5 py-0.5 text-xs text-slate-300 hover:bg-slate-700 disabled:opacity-30"
        disabled={!path}
        title="上一级"
        on:click={up}
      >
        ↑
      </button>
      <span class="min-w-0 flex-1 truncate text-[11px] text-slate-400" title={path}>
        {path || "(根目录)"}
      </span>
      <button
        class="shrink-0 rounded bg-slate-800 px-1.5 py-0.5 text-[11px] text-slate-200 hover:bg-slate-700 disabled:opacity-40"
        disabled={loading || uploading > 0}
        on:click={pickUpload}
        title="上传到当前目录"
      >
        {uploading > 0 ? `上传中(${uploading})` : "⇧ 上传"}
      </button>
      <input
        type="file"
        multiple
        class="hidden"
        bind:this={fileInput}
        on:change={uploadFiles}
      />
    </div>

    <!-- entries -->
    <div class="flex flex-1 flex-col overflow-auto p-1.5">
      {#if loading}
        <p class="px-2 py-4 text-center text-xs text-slate-500">加载中…</p>
      {:else if errMsg}
        <p class="px-2 py-3 text-center text-xs text-red-400">{errMsg}</p>
      {/if}
      {#each entries as e (e.name)}
        <div
          class="group flex w-full items-center gap-2 rounded-md px-1.5 py-1 text-sm hover:bg-slate-800"
        >
          {#if e.is_dir}
            <button class="flex min-w-0 flex-1 items-center gap-2 text-left" on:click={() => enter(e)}>
              <span class="shrink-0 text-slate-500">▾</span>
              <span class="min-w-0 flex-1 truncate text-slate-200">{e.name}/</span>
              <span class="shrink-0 text-[10px] text-slate-600">{fmtTime(e.mtime)}</span>
            </button>
          {:else if isImage(e.name)}
            <button
              class="flex min-w-0 flex-1 items-center gap-2 text-left"
              title="点击在画布中查看图片"
              on:click={() => openImageWindow(joinPath(e.name))}
            >
              <span class="shrink-0 text-slate-500">🖼</span>
              <span class="min-w-0 flex-1 truncate text-slate-200">{e.name}</span>
              <span class="shrink-0 text-[10px] tabular-nums text-slate-600">{fmtSize(e.size)}</span>
              <span class="shrink-0 text-[10px] text-slate-600">{fmtTime(e.mtime)}</span>
            </button>
            <a
              class="shrink-0 rounded px-1 text-[10px] text-slate-500 hover:bg-slate-700 hover:text-slate-200"
              title="下载"
              href={`${apiBase()}/api/file?path=${encodeURIComponent(joinPath(e.name))}`}
              download={e.name}
              on:click|stopPropagation
            >
              ⇩
            </a>
          {:else}
            <span class="w-3 shrink-0"></span>
            <span class="min-w-0 flex-1 truncate text-slate-300">{e.name}</span>
            <span class="shrink-0 text-[10px] tabular-nums text-slate-600">{fmtSize(e.size)}</span>
            <span class="shrink-0 text-[10px] text-slate-600">{fmtTime(e.mtime)}</span>
            <a
              class="shrink-0 rounded px-1 text-[10px] text-slate-500 hover:bg-slate-700 hover:text-slate-200"
              title="下载"
              href={`${apiBase()}/api/file?path=${encodeURIComponent(joinPath(e.name))}`}
              download={e.name}
              on:click|stopPropagation
            >
              ⇩
            </a>
          {/if}
        </div>
      {:else}
        {#if !loading && !errMsg}
          <p class="px-2 py-4 text-center text-xs text-slate-500">空目录</p>
        {/if}
      {/each}
    </div>
  </div>
{/if}
