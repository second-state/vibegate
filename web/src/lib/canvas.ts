/**
 * Canvas window state: which vibetty instances are open on the canvas, their
 * positions, and which one has focus (focus drives bandwidth: only the
 * focused window asks the server to push).
 */

import { derived, get, writable, type Writable } from "svelte/store";
import { arrangeNewTerminal } from "./arrange";
import type { TerminalSession } from "./session";

export type WindowRect = { x: number; y: number; w: number; h: number };

/** Default window size in world (canvas) pixels. */
export const DEFAULT_W = 720;
export const DEFAULT_H = 440;
export const MIN_W = 320;
export const MIN_H = 200;

/** All open windows, by instance prefix. */
export const windows: Writable<Map<string, WindowRect>> = writable(new Map());

/** Window keys restored from the saved layout: their terminals start
 *  disconnected (mask + click to reconnect) instead of auto-attaching. */
export const restoredKeys = new Set<string>();

const LAYOUT_KEY = "vibeview.layout";
let saveTimer: ReturnType<typeof setTimeout> | null = null;
// Dragging fires moveWindow per pointermove — debounce the localStorage
// write to a trailing tick.
windows.subscribe((m) => {
  if (saveTimer !== null) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    try {
      const data = [...m.entries()]
        .filter(([key]) => !key.startsWith("blob:")) // in-memory only, nothing to restore
        .map(([key, r]) => ({ key, ...r }));
      localStorage.setItem(LAYOUT_KEY, JSON.stringify(data));
    } catch {
      /* private mode / quota */
    }
  }, 250);
});
// Restore the previous layout: windows come back at their old spots but
// stay disconnected until clicked (restoredKeys drives that).
try {
  const saved = JSON.parse(localStorage.getItem(LAYOUT_KEY) ?? "[]");
  if (Array.isArray(saved) && saved.length) {
    const m = new Map<string, WindowRect>();
    for (const s of saved) {
      if (
        typeof s?.key === "string" &&
        [s.x, s.y, s.w, s.h].every((n) => Number.isFinite(n))
      ) {
        m.set(s.key, { x: s.x, y: s.y, w: s.w, h: s.h });
        restoredKeys.add(s.key);
      }
    }
    windows.set(m);
  }
} catch {
  /* bad json / private mode */
}

/** The focused window's prefix, or null. */
export const focusPrefix: Writable<string | null> = writable(null);

/** Whether the session drawer panel is visible. */
export const drawerOpen: Writable<boolean> = writable(true);

/** Whether the file browser panel is visible. */
export const filesOpen: Writable<boolean> = writable(false);

/** Sessions drawer keyboard-navigation state (canvas-mode `e`). */
export const drawerKbd: Writable<boolean> = writable(false);
export const drawerSel: Writable<string | null> = writable(null);
export const drawerRows: Writable<string[]> = writable([]);

/** Files drawer keyboard-navigation state (canvas-mode `f`). */
export const filesKbd: Writable<boolean> = writable(false);
export const filesSel: Writable<string | null> = writable(null);
export const filesRows: Writable<string[]> = writable([]);

/** Canvas-only mode: windows become pointer-transparent, so every click /
 *  touch reaches the canvas for panning & zooming (a "hand tool"). */
export const canvasOnly: Writable<boolean> = writable(false);

/** Prefix marking canvas windows that view a file (path follows the prefix). */
export const IMAGE_KEY_PREFIX = "img:";
export const MD_KEY_PREFIX = "md:";
export const VIDEO_KEY_PREFIX = "video:";

/** In-memory images (e.g. terminal screenshots) shown in image windows
 *  without a gateway round-trip; keyed by their "blob:" window key. */
export const blobImages: Writable<Map<string, { name: string; url: string }>> =
  writable(new Map());

/** Open an image window from an in-memory image (client-side only, nothing
 *  is stored on the gateway machine). */
export function openImageBlob(name: string, dataUrl: string, w: number, h: number) {
  const key = `blob:${name}-${Date.now()}`;
  blobImages.update((m) => new Map(m).set(key, { name, url: dataUrl }));
  const map = get(windows);
  const pos = arrangeNewTerminal([...map.values()]);
  windows.set(new Map(map).set(key, { x: pos.x, y: pos.y, w, h }));
  focusPrefix.set(key);
  revealWindow({ ...pos, w, h });
}

/** The file drawer's current directory — pasted images land here. */
export const filesDir: Writable<string> = writable("");

/** Set to a path to open the file drawer there (e.g. a terminal's cwd);
 *  the drawer consumes it and resets to null. */
export const filesNavigate: Writable<string | null> = writable(null);

/** Move the camera (keeping the current zoom) so `rect` is centered in the
 *  viewport. screen = anchor + zoom * (world − center), so the world point
 *  that must sit at the anchor (50vw−380, 50vh−260) is center + anchor/zoom. */
export function centerCameraOn(rect: WindowRect) {
  const zt = get(zoomController);
  if (!zt) return;
  zt.moveTo(
    [rect.x + rect.w / 2 - ANCHOR_X / zt.zoom, rect.y + rect.h / 2 - ANCHOR_Y / zt.zoom],
    zt.zoom,
  );
}

/** Pan the camera so `rect` is centered — skipped when it is already fully
 *  visible in the viewport. */
export function revealWindow(rect: WindowRect) {
  const zt = get(zoomController);
  if (!zt) return;
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const ax = vw / 2 - ANCHOR_X;
  const ay = vh / 2 - ANCHOR_Y;
  const x0 = zt.center[0] - ax / zt.zoom;
  const x1 = zt.center[0] + (vw - ax) / zt.zoom;
  const y0 = zt.center[1] - ay / zt.zoom;
  const y1 = zt.center[1] + (vh - ay) / zt.zoom;
  const visible =
    rect.x >= x0 &&
    rect.x + rect.w <= x1 &&
    rect.y >= y0 &&
    rect.y + rect.h <= y1;
  if (!visible) centerCameraOn(rect);
}

/** Open a viewer window (image or markdown) for a file path, reusing an
 *  existing window for the same path. */
function openFileWindow(prefix: string, w: number, h: number) {
  const map = get(windows);
  let rect: WindowRect;
  if (map.has(prefix)) {
    rect = map.get(prefix)!;
  } else {
    rect = { x: 0, y: 0, w, h };
    const pos = arrangeNewTerminal([...map.values()]);
    rect.x = pos.x;
    rect.y = pos.y;
    windows.set(new Map(map).set(prefix, rect));
  }
  focusPrefix.set(prefix);
  revealWindow(rect);
}

/** Open an image viewer window for a file path. */
export function openImageWindow(path: string) {
  openFileWindow(IMAGE_KEY_PREFIX + path, 640, 480);
}

/** Open a video player window for a file path. */
export function openVideoWindow(path: string) {
  openFileWindow(VIDEO_KEY_PREFIX + path, 720, 480);
}

/** Open a markdown viewer window for a file path. */
export function openMdWindow(path: string) {
  openFileWindow(MD_KEY_PREFIX + path, 720, 560);
}

/** The canvas TouchZoom controller (set by Canvas on mount). */
export const zoomController: Writable<{
  moveTo(pos: number[], zoom: number): Promise<void>;
  jump(pos: number[], zoom: number): void;
  center: number[];
  zoom: number;
} | null> = writable(null);

/** Window-world anchor used by the Canvas transform (see Canvas.svelte). */
export const ANCHOR_X = 380;
export const ANCHOR_Y = 260;

/** Current canvas zoom factor (mirrored from TouchZoom). */
export const canvasZoom: Writable<number> = writable(1);

/** Bumped on every canvas pan/zoom frame — lets screen-space overlays
 *  (e.g. the keybar) re-anchor to the transformed windows. */
export const canvasMoved: Writable<number> = writable(0);

/** Live sessions for open windows (component lifecycle manages entries). */
export const sessions = new Map<string, TerminalSession>();

export function registerSession(prefix: string, session: TerminalSession) {
  sessions.set(prefix, session);
}

export function unregisterSession(prefix: string) {
  sessions.delete(prefix);
}

export function getSession(prefix: string): TerminalSession | undefined {
  return sessions.get(prefix);
}

/** Open a window for an instance (no-op if already open) and focus it. */
export function openWindow(prefix: string) {
  const map = get(windows);
  let rect: WindowRect;
  if (map.has(prefix)) {
    rect = map.get(prefix)!;
  } else {
    rect = { x: 0, y: 0, w: DEFAULT_W, h: DEFAULT_H };
    const pos = arrangeNewTerminal([...map.values()]);
    rect.x = pos.x;
    rect.y = pos.y;
    windows.set(new Map(map).set(prefix, rect));
  }
  focusPrefix.set(prefix);
  revealWindow(rect);
}

/** Close a window (the owning component tears down its session). */
export function closeWindow(prefix: string) {
  const map = get(windows);
  if (!map.has(prefix)) return;
  const next = new Map(map);
  next.delete(prefix);
  windows.set(next);
  if (prefix.startsWith("blob:")) {
    blobImages.update((m) => {
      const n = new Map(m);
      n.delete(prefix);
      return n;
    });
  }
  if (get(focusPrefix) === prefix) {
    focusPrefix.set(next.keys().next().value ?? null);
  }
}

export function moveWindow(prefix: string, rect: WindowRect) {
  const map = get(windows);
  if (!map.has(prefix)) return;
  windows.set(new Map(map).set(prefix, rect));
}

/** Move the focused window to the front of the z-order (svelte each order). */
export const orderedPrefixes = derived(
  [windows, focusPrefix],
  ([map, focus]) => {
    const keys = [...map.keys()];
    if (focus && keys.includes(focus)) {
      keys.splice(keys.indexOf(focus), 1);
      keys.push(focus); // render last = on top
    }
    return keys;
  },
);

/** True when no windows are open — the canvas shows a hint instead. */
export const hasWindows = derived(windows, (map) => map.size > 0);
