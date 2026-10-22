/**
 * Sticky modifiers shared by the keybar buttons and the terminal input path:
 * latch Ctrl/Alt/Shift with one tap, the next keypress (a keybar button or a
 * soft-keyboard character) carries them, then they clear.
 */
import { writable } from "svelte/store";

export type Mods = { ctrl: boolean; alt: boolean; shift: boolean };

export const stickyMods = writable<Mods>({ ctrl: false, alt: false, shift: false });

export function toggleMod(k: keyof Mods) {
  stickyMods.update((m) => ({ ...m, [k]: !m[k] }));
}

export function clearMods() {
  stickyMods.set({ ctrl: false, alt: false, shift: false });
}

/** Encode bytes with xterm-style modifier parameters. */
export function applyMods(bytes: string, m: Mods): string {
  const mod = 1 + (m.shift ? 1 : 0) + (m.alt ? 2 : 0) + (m.ctrl ? 4 : 0);
  // Arrow keys: CSI 1;<mod><letter>
  const csi = /^(\x1b\[)([ABCD])$/.exec(bytes);
  if (csi) return `\x1b[1;${mod}${csi[2]}`;
  switch (bytes) {
    case "\t":
      return m.shift && !m.alt && !m.ctrl ? "\x1b[Z" : `\x1b[27;${mod}~`;
    case "\r":
    case "\x7f":
      return `\x1b[27;${mod}~`;
    case "\x1b":
      return "\x1b\x1b";
    default: {
      // Single printable char: ctrl maps to control codes, alt = ESC prefix.
      if (bytes.length === 1) {
        let out = bytes;
        if (m.ctrl) {
          const code = out.toUpperCase().charCodeAt(0);
          if (code >= 64 && code <= 95) out = String.fromCharCode(code & 0x1f);
        }
        return (m.alt ? "\x1b" : "") + out;
      }
      return bytes;
    }
  }
}

/** Encode soft-keyboard input with the latched sticky modifiers. Mobile
 *  keyboards have no Ctrl/Alt keys — exactly what the sticky mods are for —
 *  while Shift is already baked into the typed character itself. */
export function applyModsToTyped(data: string, m: Mods): string {
  if (!m.ctrl && !m.alt) return data;
  // Escape-sequence bursts (paste, bracketed-paste wrappers) pass verbatim —
  // ctrl-mapping them would mangle the CSI bytes.
  if (data.includes("\x1b")) return data;
  return [...data]
    .map((ch) => applyMods(ch, { ctrl: m.ctrl, alt: m.alt, shift: false }))
    .join("");
}
