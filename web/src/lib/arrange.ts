/**
 * Position for a new canvas window that does not intersect existing ones.
 * Ported from sshx <https://github.com/ekzhang/sshx> (MIT) — golden-angle
 * spiral placement.
 */

const PLACE_W = 752;
const PLACE_H = 475;
const PLACE_PAD = 16;

type ExistingWindow = {
  x: number;
  y: number;
  w: number;
  h: number;
};

/** Choose a position for a new window that does not intersect existing ones. */
export function arrangeNewTerminal(existing: ExistingWindow[]) {
  if (existing.length === 0) {
    return { x: 0, y: 0 };
  }

  const startX = 100 * (Math.random() - 0.5);
  const startY = 60 * (Math.random() - 0.5);

  for (let i = 0; ; i++) {
    const t = 1.94161103872 * i;
    const x = Math.round(startX + 8 * i * Math.cos(t));
    const y = Math.round(startY + 8 * i * Math.sin(t));
    let ok = true;
    for (const box of existing) {
      if (
        isect(x, x + PLACE_W, box.x, box.x + box.w) &&
        isect(y, y + PLACE_H, box.y, box.y + box.h)
      ) {
        ok = false;
        break;
      }
    }
    if (ok) {
      return { x, y };
    }
  }
}

function isect(s1: number, e1: number, s2: number, e2: number): boolean {
  return s1 - PLACE_PAD < e2 && e1 + PLACE_PAD > s2;
}
