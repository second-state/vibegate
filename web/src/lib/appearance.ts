/**
 * Appearance settings: terminal color preset, canvas/terminal background
 * images. Persisted to localStorage (per device).
 */

import { writable } from "svelte/store";
import { connection } from "./connection";

export type Appearance = {
  /** Key into TERM_PRESETS. */
  theme: string;
  /** Background image for the canvas: file path or http(s) URL. Empty = none. */
  canvasBg: string;
  /** Background image for terminal windows: file path or http(s) URL. Empty = none. */
  termBg: string;
  /** Opacity of the terminal background, 0–100 (100 = fully opaque): the
   *  image overlay when termBg is set, else the background color itself
   *  (fading it reveals the canvas background behind the pane). */
  termBgOpacity: number;
  /** Keybar placement: float under the focused window, docked on top, hidden. */
  keybar: "float" | "top" | "hidden";
  /** Window chrome style: classic / glass / cyber. */
  chrome: "classic" | "glass" | "cyber";
};

const DEFAULTS: Appearance = {
  theme: "tokyo-night",
  canvasBg: "",
  termBg: "",
  termBgOpacity: 100,
  keybar: "float",
  chrome: "classic",
};
const KEY = "vibeview.appearance";

function load(): Appearance {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) {
      const a = { ...DEFAULTS, ...JSON.parse(raw) } as Appearance;
      // sanitize removed styles (e.g. "hud") back to the default
      if (!["classic", "glass", "cyber"].includes(a.chrome)) a.chrome = "classic";
      return a;
    }
  } catch {
    /* private mode / bad json */
  }
  return { ...DEFAULTS };
}

let current: Appearance = load();

export const appearance = writable<Appearance>(current);

appearance.subscribe((v) => {
  current = v;
  try {
    localStorage.setItem(KEY, JSON.stringify(v));
  } catch {
    /* ignore */
  }
});

/** Current value without subscribing. */
export function getAppearance(): Appearance {
  return current;
}

export function setAppearance(patch: Partial<Appearance>) {
  appearance.update((a) => ({ ...a, ...patch }));
}

/**
 * Resolve a background value to a fetchable URL: http(s) URLs pass through,
 * anything else is treated as a file path served by the gateway.
 */
export function resolveBgUrl(value: string): string {
  if (!value) return "";
  if (/^https?:\/\//i.test(value)) return value;
  return `${connection.apiBase}/api/file?path=${encodeURIComponent(value)}`;
}
