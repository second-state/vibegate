/**
 * Tab attention: when the page is hidden, start watching the canvas —
 * panes that *become* done/blocked while the user is away (i.e. not
 * already stopped at the moment of hiding) make the tab call for
 * attention: the axolotl favicon bounces and the title gets "[n]"
 * (n = newly stopped panes). Only canvas-open windows count. Returning
 * to the page clears everything; nothing happens before the first hide.
 */

import { derived, get } from "svelte/store";
import { connection } from "./connection";
import { windows } from "./canvas";

const BASE_TITLE = document.title;
const STOPPED = new Set(["done", "blocked"]);

/** Snapshot (sids) of canvas panes already stopped when we hid — the
 *  baseline. null while visible / before the first hide. */
let baseline: Set<string> | null = null;

/** Canvas-open panes currently stopped. */
const canvasStopped = derived(
  [connection.panes, windows],
  ([panes, wins]) =>
    panes.filter((p) => wins.has(p.sid) && STOPPED.has(p.agent_status)),
);

const link =
  document.querySelector<HTMLLinkElement>("link[rel*='icon']") ??
  (() => {
    const l = document.createElement("link");
    l.rel = "icon";
    document.head.appendChild(l);
    return l;
  })();
const STATIC_HREF = link.href;

/** Favicon frames for the bounce: pre-rendered data URLs, favicon swapped
 *  in a small interval while alerting (the classic canvas-favicon trick —
 *  an SVG href can't be animated in place). */
let frames: string[] = [];
let timer: number | null = null;
let frameIdx = 0;

async function buildFrames() {
  try {
    const img = new Image();
    img.src = STATIC_HREF;
    await img.decode();
    // The SVG's viewBox is cropped to the drawing itself (no margins), so
    // plain aspect-preserving scaling lands the axolotl on the floor.
    const SIZE = 128;
    const c = document.createElement("canvas");
    c.width = SIZE;
    c.height = SIZE;
    const ctx = c.getContext("2d")!;
    const iw = img.width || SIZE;
    const ih = img.height || SIZE;
    const scale = Math.min(SIZE / iw, SIZE / ih);
    const dw = iw * scale;
    const dh = ih * scale;
    // Flicker: artwork (orange) → white silhouette → black silhouette.
    // `source-atop` tints only the drawing's pixels, keeping its shape.
    const tints: (string | null)[] = [null, "#ffffff", "#000000"];
    frames = tints.map((tint) => {
      ctx.clearRect(0, 0, SIZE, SIZE);
      ctx.drawImage(img, (SIZE - dw) / 2, SIZE - 6 - dh, dw, dh);
      if (tint) {
        ctx.globalCompositeOperation = "source-atop";
        ctx.fillStyle = tint;
        ctx.fillRect(0, 0, SIZE, SIZE);
        ctx.globalCompositeOperation = "source-over";
      }
      return c.toDataURL();
    });
    // If we became alerting before frames were ready, kick the loop now.
    update();
  } catch {
    /* favicon stays static; the [n] title prefix still works */
  }
}

function stop() {
  if (timer !== null) {
    clearInterval(timer);
    timer = null;
  }
  link.href = STATIC_HREF;
}

let lastN = 0;

function update() {
  if (document.hidden && baseline !== null) {
    const cur = get(canvasStopped);
    // A pane that left the stopped state is "fresh" again: drop it from
    // the hide-time baseline so a later re-entry counts as new.
    const sids = new Set(cur.map((p) => p.sid));
    for (const sid of [...baseline]) {
      if (!sids.has(sid)) baseline.delete(sid);
    }
    const n = cur.filter((p) => !baseline.has(p.sid)).length;
    if (n > 0) {
      if (n !== lastN) {
        document.title = `[${n}] ${BASE_TITLE}`;
        lastN = n;
      }
      if (timer === null && frames.length) {
        timer = setInterval(() => {
          link.href = frames[frameIdx++ % frames.length];
        }, 220);
      }
    } else {
      document.title = BASE_TITLE;
      lastN = 0;
      stop();
    }
  } else {
    document.title = BASE_TITLE;
    lastN = 0;
    stop();
  }
}

document.addEventListener("visibilitychange", () => {
  if (document.hidden) {
    // Start recording: everything already stopped right now is "seen".
    baseline = new Set(get(canvasStopped).map((p) => p.sid));
    lastN = 0;
  } else {
    baseline = null;
    document.title = BASE_TITLE;
    stop();
  }
});
canvasStopped.subscribe(update);
void buildFrames();
