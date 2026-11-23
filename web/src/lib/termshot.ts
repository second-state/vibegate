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

export type ChromeStyle = "classic" | "glass" | "cyber";

/** Colors matching the statusColor() palette in discovery.ts. */
const STATUS_HEX: Record<string, string> = {
  working: "#4ade80",
  blocked: "#fbbf24",
  done: "#38bdf8",
  idle: "#64748b",
  "": "#64748b",
};

function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace("#", "");
  const s = h.length === 3 ? h.split("").map((c) => c + c).join("") : h;
  return [
    parseInt(s.slice(0, 2), 16) || 0,
    parseInt(s.slice(2, 4), 16) || 0,
    parseInt(s.slice(4, 6), 16) || 0,
  ];
}

/** overlay(overlayHex, alpha) composited over baseHex → opaque hex. */
function blend(base: string, overlay: string, alpha: number): string {
  const [r1, g1, b1] = hexToRgb(base);
  const [r2, g2, b2] = hexToRgb(overlay);
  const mix = (a: number, b: number) => Math.round(a * (1 - alpha) + b * alpha);
  return `rgb(${mix(r1, r2)},${mix(g1, g2)},${mix(b1, b2)})`;
}

/** Screenshot the terminal: styled header (per appearance.chrome) + border
 *  around the rendered grid when `header` is on; grid-only otherwise. */
export async function termShotDataURL(
  session: TerminalSession,
  opts: { chrome: ChromeStyle; title: string; status: string; header: boolean },
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
  if (!opts.header) {
    return { dataUrl: canvas.toDataURL("image/png"), w: Math.round(W), h: Math.round(H) };
  }
  return composeWindowShot(canvas, W, H, a.chrome, opts, baseBg, dpr);
}

/** Compose the styled header + border around the grid canvas. */
function composeWindowShot(
  grid: HTMLCanvasElement,
  gridW: number,
  gridH: number,
  chrome: ChromeStyle,
  opts: { title: string; status: string },
  baseBg: string,
  dpr: number,
): { dataUrl: string; w: number; h: number } {
  const HEAD_H = 32;
  const R = 8;
  const PAD = 1; // border width
  const W = gridW + PAD * 2;
  const H = HEAD_H + gridH + PAD * 2;
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(W * dpr);
  canvas.height = Math.round(H * dpr);
  const ctx = canvas.getContext("2d")!;
  ctx.scale(dpr, dpr);

  // rounded window clip
  const rr = () => {
    ctx.beginPath();
    ctx.roundRect(0.5, 0.5, W - 1, H - 1, R);
  };
  ctx.save();
  rr();
  ctx.clip();

  // base
  ctx.fillStyle = baseBg;
  ctx.fillRect(0, 0, W, H);

  // header background per style
  if (chrome === "classic") {
    ctx.fillStyle = "#1f2233";
  } else if (chrome === "glass") {
    ctx.fillStyle = blend(baseBg, "#ffffff", 0.07);
  } else {
    // cyber: cyan→magenta wash
    ctx.fillStyle = blend(baseBg, "#22d3ee", 0.1);
    ctx.fillRect(0, 0, W, HEAD_H);
    const g = ctx.createLinearGradient(0, 0, W, 0);
    g.addColorStop(0, "rgba(34,211,238,0.10)");
    g.addColorStop(1, "rgba(217,70,239,0.12)");
    ctx.fillStyle = g;
  }
  ctx.fillRect(0, 0, W, HEAD_H);
  // header separator
  ctx.fillStyle = "rgba(30,41,59,1)";
  ctx.fillRect(0, HEAD_H - 1, W, 1);

  // status dot + title
  const dotColor = STATUS_HEX[opts.status] ?? STATUS_HEX[""];
  ctx.fillStyle = dotColor;
  ctx.beginPath();
  ctx.arc(12, HEAD_H / 2, 4, 0, Math.PI * 2);
  ctx.fill();
  ctx.font = "600 12px ui-sans-serif, system-ui, sans-serif";
  ctx.textBaseline = "middle";
  ctx.textAlign = "left";
  ctx.fillStyle = "#cbd5e1";
  ctx.fillText(opts.title, 24, HEAD_H / 2);

  // grid below the header
  ctx.drawImage(grid, PAD, PAD + HEAD_H, gridW, gridH);
  ctx.restore();

  // window border per style
  ctx.save();
  rr();
  if (chrome === "glass") {
    ctx.strokeStyle = "rgba(255,255,255,0.4)";
    ctx.lineWidth = 1;
    ctx.stroke();
    ctx.shadowColor = "rgba(56,189,248,0.35)";
    ctx.shadowBlur = 14;
    ctx.stroke();
  } else if (chrome === "cyber") {
    ctx.strokeStyle = "#22d3ee";
    ctx.lineWidth = 1;
    ctx.shadowColor = "rgba(34,211,238,0.45)";
    ctx.shadowBlur = 10;
    ctx.stroke();
    ctx.shadowBlur = 0;
    // chromatic offset edges
    ctx.strokeStyle = "rgba(217,70,239,0.4)";
    ctx.beginPath();
    ctx.roundRect(3.5, 3.5, W - 7, H - 7, R - 2);
    ctx.stroke();
  } else {
    ctx.strokeStyle = "#64748b";
    ctx.lineWidth = 1;
    ctx.stroke();
  }
  ctx.restore();
  return { dataUrl: canvas.toDataURL("image/png"), w: Math.round(W), h: Math.round(H) };
}
