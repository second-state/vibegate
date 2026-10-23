<script lang="ts">
  import { filesOpen, drawerOpen, openImageWindow, openMdWindow } from "../lib/canvas";
  import { connection } from "../lib/connection";
  import { get } from "svelte/store";
  import { t } from "../lib/i18n";

  type Entry = {
    name: string;
    is_dir: boolean;
    size: number;
    mtime?: number | null;
  };

  const IMAGE_EXT = /\.(png|jpe?g|gif|webp|svg|bmp|avif)$/i;
  const MD_EXT = /\.(md|markdown)$/i;
  const isImage = (name: string) => IMAGE_EXT.test(name);
  const isMd = (name: string) => MD_EXT.test(name);

  let path = ""; // "" = gateway file root
  let entries: Entry[] = [];
  let loading = false;
  let errMsg = "";

  type Upload = { id: number; name: string; pct: number };
  let uploads: Upload[] = []; // in-flight uploads, shown as progress rows
  let uploadSeq = 0;
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
        if (body.type === "file") errMsg = get(t)("isFile");
        return;
      }
      if (body.type !== "dir") {
        errMsg = get(t)("notDir");
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

  /** PUT with upload progress — fetch can't report it, XHR can. Resumable:
   *  sends the slice from `offset` with the token; the server appends. */
  function putWithProgress(
    target: string,
    file: File,
    offset: number,
    token: string,
    onProgress: (pct: number) => void,
  ): Promise<void> {
    return new Promise((resolve, reject) => {
      const xhr = new XMLHttpRequest();
      xhr.open("PUT", `${apiBase()}/api/file?path=${encodeURIComponent(target)}`);
      xhr.setRequestHeader("X-Upload-Token", token);
      xhr.setRequestHeader("X-Upload-Total", String(file.size));
      xhr.upload.onprogress = (e) => {
        if (e.lengthComputable) {
          onProgress(Math.min(100, Math.round(((offset + e.loaded) / file.size) * 100)));
        }
      };
      xhr.onload = () => {
        if (xhr.status >= 200 && xhr.status < 300) {
          resolve();
          return;
        }
        let msg = `HTTP ${xhr.status}`;
        try {
          msg = JSON.parse(xhr.responseText)?.error ?? msg;
        } catch {
          /* non-JSON error body */
        }
        reject(new Error(msg));
      };
      xhr.onerror = () => reject(new Error(get(t)("netError")));
      xhr.send(offset > 0 ? file.slice(offset) : file);
    });
  }

  /** Stable token per target path, so interrupted uploads stay resumable
   *  across retries and even page reloads. */
  const TOKEN_KEY = "vibeview.uploadTokens";
  /** randomUUID needs a secure context; getRandomValues works everywhere. */
  function randomToken(): string {
    const b = new Uint8Array(8);
    crypto.getRandomValues(b);
    return Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
  }
  function tokenFor(target: string): string {
    try {
      const m = JSON.parse(localStorage.getItem(TOKEN_KEY) ?? "{}");
      if (typeof m[target] === "string") return m[target];
      const t = randomToken();
      m[target] = t;
      localStorage.setItem(TOKEN_KEY, JSON.stringify(m));
      return t;
    } catch {
      return randomToken();
    }
  }

  /** Bytes the server already holds for this token. Never guesses: if the
   *  query fails we must not assume 0 — sending the whole file would make
   *  the server append it onto the existing partial. */
  async function uploadOffset(target: string, token: string): Promise<number> {
    const resp = await fetch(
      `${apiBase()}/api/upload/status?path=${encodeURIComponent(target)}&token=${token}`,
    );
    const body = await resp.json();
    if (typeof body.size !== "number") throw new Error(get(t)("breakpointQueryError"));
    return body.size;
  }

  /** One file, resumable: up to 3 attempts, each continuing from the bytes
   *  already on the server. */
  async function uploadOne(file: File): Promise<void> {
    const id = ++uploadSeq;
    const target = joinPath(file.name);
    const token = tokenFor(target);
    uploads = [...uploads, { id, name: file.name, pct: 0 }];
    try {
      for (let attempt = 1; ; attempt++) {
        try {
          const offset = Math.min(await uploadOffset(target, token), file.size);
          uploads = uploads.map((u) =>
            u.id === id ? { ...u, pct: Math.round((offset / file.size) * 100) } : u,
          );
          await putWithProgress(target, file, offset, token, (pct) => {
            uploads = uploads.map((u) => (u.id === id ? { ...u, pct } : u));
          });
          return;
        } catch (e) {
          // Covers both the PUT and the breakpoint query (which must never
          // be skipped — assuming offset 0 could duplicate data server-side).
          if (attempt >= 3) throw e;
          errMsg = get(t)("resumeIn", { n: attempt });
          await new Promise((r) => setTimeout(r, 1000));
        }
      }
    } finally {
      uploads = uploads.filter((u) => u.id !== id);
    }
  }

  /** Files that failed all attempts — a ↻ button resumes them in place
   *  from the breakpoint (no re-picking). */
  let failedUploads: { id: number; file: File }[] = [];
  let failedSeq = 0;

  async function retryUpload(entry: { id: number; file: File }) {
    failedUploads = failedUploads.filter((x) => x.id !== entry.id);
    try {
      await uploadOne(entry.file);
    } catch (e) {
      failedUploads = [...failedUploads, entry];
      errMsg = get(t)("uploadFailed", { name: entry.file.name, msg: (e as Error).message });
      return;
    }
    await load(path);
  }

  async function uploadFiles(ev: Event) {
    const input = ev.target as HTMLInputElement;
    const files = Array.from(input.files ?? []);
    input.value = "";
    for (const f of files) {
      try {
        await uploadOne(f);
      } catch (e) {
        failedUploads = [...failedUploads, { id: ++failedSeq, file: f }];
        errMsg = get(t)("uploadFailed", { name: f.name, msg: (e as Error).message });
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
    class="absolute inset-y-2 right-2 z-10 flex w-80 max-w-[calc(100vw-1rem)] flex-col overflow-hidden rounded-xl border border-slate-700 bg-slate-900/95 shadow-2xl backdrop-blur"
  >
    <div class="flex items-center justify-between border-b border-slate-800 px-3 py-2">
      <span class="text-xs font-semibold text-slate-300">{$t("files")}</span>
      <button
        class="rounded px-1 text-xs text-slate-500 hover:text-slate-200"
        title={$t("refresh")}
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
        title={$t("goUp")}
        on:click={up}
      >
        ↑
      </button>
      <span class="min-w-0 flex-1 truncate text-[11px] text-slate-400" title={path}>
        {path || $t("rootDir")}
      </span>
      <button
        class="shrink-0 rounded bg-slate-800 px-1.5 py-0.5 text-[11px] text-slate-200 hover:bg-slate-700 disabled:opacity-40"
        disabled={loading || uploads.length > 0}
        on:click={pickUpload}
        title={$t("uploadTitle")}
      >
        {uploads.length > 0 ? $t("uploading", { n: uploads.length }) : $t("uploadLabel")}
      </button>
      <input
        type="file"
        multiple
        class="hidden"
        bind:this={fileInput}
        on:change={uploadFiles}
      />
    </div>

    <!-- upload progress + failed rows -->
    {#if uploads.length || failedUploads.length}
      <div class="flex flex-col gap-1 border-b border-slate-800 px-3 py-1.5">
        {#each uploads as u (u.id)}
          <div class="flex items-center gap-2 text-[11px] text-slate-400">
            <span class="min-w-0 flex-1 truncate">{u.name}</span>
            <span class="shrink-0 tabular-nums text-slate-500">{u.pct}%</span>
          </div>
          <div class="h-1 w-full overflow-hidden rounded bg-slate-800">
            <div class="h-full bg-blue-500" style:width="{u.pct}%"></div>
          </div>
        {/each}
        {#each failedUploads as fu (fu.id)}
          <div class="flex items-center gap-2 text-[11px]">
            <span class="min-w-0 flex-1 truncate text-red-300">⚠ {fu.file.name}</span>
            <button
              class="shrink-0 rounded bg-slate-800 px-1.5 py-0.5 text-slate-200 hover:bg-slate-700"
              title={$t("resumeUpload")}
              on:click={() => retryUpload(fu)}
            >
              ↻ {$t("retryUpload")}
            </button>
            <button
              class="shrink-0 px-0.5 text-slate-500 hover:text-slate-200"
              title={$t("dismiss")}
              on:click={() => (failedUploads = failedUploads.filter((x) => x.id !== fu.id))}
            >
              ×
            </button>
          </div>
        {/each}
      </div>
    {/if}

    <!-- entries -->
    <div class="flex flex-1 flex-col overflow-auto p-1.5">
      {#if loading}
        <p class="px-2 py-4 text-center text-xs text-slate-500">{$t("loading")}</p>
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
          {:else if isMd(e.name)}
            <button
              class="flex min-w-0 flex-1 items-center gap-2 text-left"
              title={$t("viewMd")}
              on:click={() => openMdWindow(joinPath(e.name))}
            >
              <span class="shrink-0 text-slate-500">📝</span>
              <span class="min-w-0 flex-1 truncate text-slate-200">{e.name}</span>
              <span class="shrink-0 text-[10px] tabular-nums text-slate-600">{fmtSize(e.size)}</span>
              <span class="shrink-0 text-[10px] text-slate-600">{fmtTime(e.mtime)}</span>
            </button>
            <a
              class="shrink-0 rounded px-1 text-[10px] text-slate-500 hover:bg-slate-700 hover:text-slate-200"
              title={$t("download")}
              href={`${apiBase()}/api/file?path=${encodeURIComponent(joinPath(e.name))}`}
              download={e.name}
              on:click|stopPropagation
            >
              ⇩
            </a>
          {:else if isImage(e.name)}
            <button
              class="flex min-w-0 flex-1 items-center gap-2 text-left"
              title={$t("viewImage")}
              on:click={() => openImageWindow(joinPath(e.name))}
            >
              <span class="shrink-0 text-slate-500">🖼</span>
              <span class="min-w-0 flex-1 truncate text-slate-200">{e.name}</span>
              <span class="shrink-0 text-[10px] tabular-nums text-slate-600">{fmtSize(e.size)}</span>
              <span class="shrink-0 text-[10px] text-slate-600">{fmtTime(e.mtime)}</span>
            </button>
            <a
              class="shrink-0 rounded px-1 text-[10px] text-slate-500 hover:bg-slate-700 hover:text-slate-200"
              title={$t("download")}
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
              title={$t("download")}
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
          <p class="px-2 py-4 text-center text-xs text-slate-500">{$t("emptyDir")}</p>
        {/if}
      {/each}
    </div>
  </div>
{/if}
