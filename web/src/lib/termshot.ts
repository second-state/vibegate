/**
 * Terminal screenshot: render the visible xterm grid to a PNG data URL.
 * The buffer holds structured cells (char/fg/bg), so this is vector text
 * drawn at device resolution — far crisper than any pixel capture, and
 * naturally scoped to exactly one window.
 *
 * Mirrors what the window shows: the chosen theme preset's background as
 * the base, then the terminal background image (cover, opacity) — cells
 * with a default background stay transparent so the image shows through,
 * matching xterm's allowTransparency behavior.
 */
import type { TerminalSession } from "./session";
import { getAppearance, resolveBgUrl } from "./appearance";
import { TERM_PRESETS } from "./theme";

// xterm's 256-color palette: 0-15 resolve from the theme; 16-231 are a
// 6×6×6 cube; 232-255 a gray ramp.
const CUBE = [0, 95, 135, 175, 215, 255];
function palette256(i: number): string {
  if (i < 16) return "";
  if (i < 232) {
    const n = i - 16;
    return `rgb(${CUBE[Math.floor(n / 36)]},${CUBE[Math.floor(n / 6) % 6]},${CUBE[n % 6]})`;
  }
  const g = 8 + (i - 232) * 10;
  return `rgb(${g},${g},${g})`;
}

const BASE16 = [
  "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
  "brightBlack", "brightRed", "brightGreen", "brightYellow", "brightBlue",
  "brightMagenta", "brightCyan", "brightWhite",
];

function color16(i: number, theme: Record<string, string>): string {
  return theme[BASE16[i] ?? ""] ?? palette256(i);
}

/** Cell color; `null` = "default" (no paint — background image shows). */
function cellColor(
  cell: { getFgColorMode(): number; getFgColor(): number },
  isFg: boolean,
  theme: Record<string, string>,
  fallback: string,
): string | null {
  const mode = isFg ? cell.getFgColorMode() : cell.getBgColorMode();
  const v = isFg ? cell.getFgColor() : cell.getBgColor();
  switch (mode) {
    case 0:
      return isFg ? (theme.foreground ?? fallback) : null;
    case 1:
      return color16(v, theme);
    case 2:
      return palette256(v);
    default: {
      const r = (v >> 16) & 255;
      const g = (v >> 8) & 255;
      const b = v & 255;
      return `rgb(${r},${g},${b})`;
    }
  }
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("bg image failed"));
    img.src = src;
  });
}

/** Render the visible grid; returns a PNG data URL plus its CSS pixel size. */
export async function termShotDataURL(
  session: TerminalSession,
): Promise<{ dataUrl: string; w: number; h: number } | null> {
  const term = session.term;
  // Same private renderer-dims API the scrollbar fix and mouse tracking use.
  const dims = (term as any)._core?._renderService?.dimensions?.css?.cell;
  if (!dims?.width || !dims?.height) return null;
  const a = getAppearance();
  const preset = TERM_PRESETS[a.theme] ?? TERM_PRESETS["tokyo-night"];
  // The live theme's background is transparent when a bg image is on —
  // use the preset's opaque color as the base layer.
  const baseBg = preset.theme.background ?? "#1a1b26";
  const theme = (term.options.theme ?? {}) as Record<string, string>;
  const dpr = Math.min(window.devicePixelRatio || 1, 3);
  const cw = dims.width;
  const ch = dims.height;
  const W = term.cols * cw;
  const H = term.rows * ch;
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(W * dpr);
  canvas.height = Math.round(H * dpr);
  const ctx = canvas.getContext("2d")!;
  ctx.scale(dpr, dpr);
  ctx.fillStyle = baseBg;
  ctx.fillRect(0, 0, W, H);
  if (a.termBg) {
    try {
      const img = await loadImage(resolveBgUrl(a.termBg));
      ctx.save();
      ctx.globalAlpha = Math.min(100, Math.max(0, a.termBgOpacity ?? 100)) / 100;
      // cover-fit, like bg-cover in the window
      const scale = Math.max(W / img.width, H / img.height);
      const dw = img.width * scale;
      const dh = img.height * scale;
      ctx.drawImage(img, (W - dw) / 2, (H - dh) / 2, dw, dh);
      ctx.restore();
    } catch {
      /* bg image unavailable — solid preset background stands */
    }
  }
  ctx.textBaseline = "middle";
  ctx.textAlign = "center";

  const buf = term.buffer.active;
  const top = buf.viewportY; // absolute index of the first visible line
  for (let y = 0; y < term.rows; y++) {
    const line = buf.getLine(top + y);
    if (!line) continue;
    for (let x = 0; x < term.cols; x++) {
      const cell = line.getCell(x);
      if (!cell || cell.getWidth() === 0) continue; // wide-char continuation
      let fg = cellColor(cell, true, theme, theme.foreground ?? "#c0caf5");
      let bg = cellColor(cell, false, theme, baseBg);
      if (cell.isInverse()) {
        [fg, bg] = [bg, fg];
        // inverse with a default color: paint an opaque background so the
        // text stays visible over the bg image
        if (bg === null) bg = theme.foreground ?? fg ?? "#c0caf5";
        if (fg === null) fg = baseBg;
      }
      const wide = cell.getWidth() > 1;
      if (bg !== null) {
        ctx.fillStyle = bg;
        ctx.fillRect(x * cw, y * ch, cw * (wide ? 2 : 1), ch);
      }
      const chars = cell.getChars();
      if (!chars) continue;
      ctx.font = `${cell.isBold() ? "bold " : ""}${term.options.fontSize ?? 14}px ${
        term.options.fontFamily ?? "monospace"
      }`;
      ctx.fillStyle = fg ?? "#c0caf5";
      ctx.fillText(chars, x * cw + (cw * (wide ? 2 : 1)) / 2, y * ch + ch / 2);
    }
  }
  return { dataUrl: canvas.toDataURL("image/png"), w: Math.round(W), h: Math.round(H) };
}
