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

/** The focused window's prefix, or null. */
export const focusPrefix: Writable<string | null> = writable(null);

/** Whether the session drawer panel is visible. */
export const drawerOpen: Writable<boolean> = writable(true);

/** Whether the file browser panel is visible. */
export const filesOpen: Writable<boolean> = writable(false);

/** Prefix marking canvas windows that view a file (path follows the prefix). */
export const IMAGE_KEY_PREFIX = "img:";

/** Open an image viewer window for a file path (reuses the existing window
 *  when the same path is already open) and focus it. */
export function openImageWindow(path: string) {
  const key = IMAGE_KEY_PREFIX + path;
  const map = get(windows);
  if (!map.has(key)) {
    const rect = { x: 0, y: 0, w: 640, h: 480 };
    const pos = arrangeNewTerminal([...map.values()]);
    rect.x = pos.x;
    rect.y = pos.y;
    windows.set(new Map(map).set(key, rect));
  }
  focusPrefix.set(key);
}

/** The canvas TouchZoom controller (set by Canvas on mount). */
export const zoomController: Writable<{
  moveTo(pos: number[], zoom: number): Promise<void>;
  center: number[];
  zoom: number;
} | null> = writable(null);

/** Current canvas zoom factor (mirrored from TouchZoom). */
export const canvasZoom: Writable<number> = writable(1);

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
  if (!map.has(prefix)) {
    const rect = { x: 0, y: 0, w: DEFAULT_W, h: DEFAULT_H };
    const pos = arrangeNewTerminal([...map.values()]);
    rect.x = pos.x;
    rect.y = pos.y;
    windows.set(new Map(map).set(prefix, rect));
  }
  focusPrefix.set(prefix);
}

/** Close a window (the owning component tears down its session). */
export function closeWindow(prefix: string) {
  const map = get(windows);
  if (!map.has(prefix)) return;
  const next = new Map(map);
  next.delete(prefix);
  windows.set(next);
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
