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
  /** Canvas grid backdrop style: dots / lines / cross / none. */
  canvasGrid: "dots" | "lines" | "cross" | "none";
  /** Black overlay over the canvas background image, 0–100 (100 = fully
   *  black): dims the picture so terminal windows stand out. */
  canvasBgDim: number;
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
  canvasGrid: "dots",
  canvasBgDim: 55,
  termBg: "",
  termBgOpacity: 100,
  keybar: "float",
  chrome: "classic",
};
const KEY = "vibefleet.appearance";

function load(): Appearance {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) {
      const a = { ...DEFAULTS, ...JSON.parse(raw) } as Appearance;
      // sanitize removed styles (e.g. "hud") back to the default
      if (!["classic", "glass", "cyber"].includes(a.chrome)) a.chrome = "classic";
      if (!["dots", "lines", "cross", "none"].includes(a.canvasGrid)) a.canvasGrid = "dots";
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

/** `#rrggbb` → `#rrggbbaa` with alpha = opacityPct/100 (xterm's
 *  allowTransparency renders it over whatever sits behind the canvas). */
export function withAlpha(hex: string, opacityPct: number): string {
  const body = hex.replace("#", "");
  if (body.length !== 6) return hex;
  const a = Math.round((Math.min(100, Math.max(0, opacityPct)) / 100) * 255)
    .toString(16)
    .padStart(2, "0");
  return `#${body}${a}`;
}

/** Inline background/border style for the chrome surfaces (header, session
 *  and files drawers): a base fading with the terminal opacity slider —
 *  the same fade the terminals get — with the chrome style decorations
 *  layered on top. */
export function surfaceStyle(chrome: Appearance["chrome"], opacityPct: number): string {
  const base = withAlpha("#16161e", opacityPct);
  if (chrome === "glass") {
    return (
      `background:linear-gradient(180deg,rgba(255,255,255,.08),rgba(255,255,255,.02)),${base};` +
      `backdrop-filter:blur(18px) saturate(1.3);` +
      `border-color:rgba(255,255,255,.16);`
    );
  }
  if (chrome === "cyber") {
    return (
      `background:linear-gradient(90deg,rgba(34,211,238,.08),rgba(217,70,239,.08)),${base};` +
      `border-color:rgba(34,211,238,.45);`
    );
  }
  return `background:${base};`;
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
