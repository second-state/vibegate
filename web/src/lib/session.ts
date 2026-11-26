/**
 * One terminal window: a raw WebSocket to vibegate's `/term/<sid>`.
 *
 * The socket carries pure terminal bytes — vibegate attaches
 * `herdr terminal attach` for us and herdr repaints the full screen on
 * connect/resize, so there is no baseline protocol; bytes are simply appended
 * to xterm in order. Input goes back as binary frames; resize is a small
 * text frame.
 */

// SPIKE (spike/ghostty-web): libghostty WASM instead of xterm.js.
// Rendering is ghostty's canvas renderer; kitty graphics and grapheme
// clustering are native. The xterm addons (webgl/unicode/image) are gone —
// see the notes in attach().
import { init, Terminal, FitAddon } from "ghostty-web";
import { get, writable, type Writable } from "svelte/store";
import { TERM_FONT_FAMILY, TERM_FONT_SIZE, TERM_PRESETS } from "./theme";
import { appearance, getAppearance, resolveBgUrl } from "./appearance";
import { stickyMods, clearMods, applyModsToTyped } from "./mods";
import { t } from "./i18n";

const SYNC_DEBOUNCE_MS = 200;

/** WASM init promise — `new Terminal()` throws until this resolves. */
let ghosttyReady: Promise<void> | null = null;
function ghosttyInit(): Promise<void> {
  return (ghosttyReady ??= init());
}

export type SessionStatus = "connecting" | "live" | "ended" | "error";

/** Small transient toast at the top of the screen. */
export function showToast(text: string, ok = true) {
  const old = document.getElementById("vibegate-copied-toast");
  old?.remove();
  const el = document.createElement("div");
  el.id = "vibegate-copied-toast";
  el.textContent = text;
  el.style.cssText =
    "position:fixed;top:18px;left:50%;transform:translateX(-50%);z-index:9999;" +
    `background:${ok ? "#1f2233" : "#7f1d1d"};color:${ok ? "#cbd5e1" : "#fecaca"};` +
    "border:1px solid #334155;border-radius:8px;" +
    "padding:6px 14px;font-size:12px;box-shadow:0 6px 24px rgba(0,0,0,.45);" +
    "opacity:0;transition:opacity .15s";
  document.body.appendChild(el);
  requestAnimationFrame(() => (el.style.opacity = "1"));
  setTimeout(() => {
    el.style.opacity = "0";
    setTimeout(() => el.remove(), 200);
  }, ok ? 1100 : 2600);
}

/** Small toast reporting the copy-on-selection outcome. */
export function showCopiedToast(ok: boolean, msg: string) {
  showToast(ok ? get(t)("copiedToast") : `${get(t)("copyFailed")}: ${msg}`, ok);
}

/** Copy text to the clipboard: async clipboard API with the legacy
 *  execCommand fallback (insecure http contexts have no clipboard API). */
export async function copyTextToClipboard(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }
  const ta = document.createElement("textarea");
  ta.value = text;
  ta.style.cssText = "position:fixed;top:0;left:0;opacity:0";
  document.body.appendChild(ta);
  ta.focus();
  ta.select();
  const ok = document.execCommand("copy");
  ta.remove();
  if (!ok) throw new Error("execCommand copy returned false");
}

export class TerminalSession {
  readonly prefix: string; // kept for compatibility: the herdr terminal id
  /** Created in attach() (needs the async WASM init); any-typed: the
   *  ghostty-web compat surface is duck-typed for this spike. */
  term: any = null;
  fitAddon: any = null;

  readonly status: Writable<SessionStatus> = writable("connecting");
  /** Current terminal grid size, mirrored from the last fit/sync. */
  readonly size: Writable<{ cols: number; rows: number }> = writable({ cols: 80, rows: 24 });
  private set_status(v: SessionStatus) { this.statusValue = v; this.status.set(v); }

  private ws: WebSocket | null = null;
  private gatewayUrl: string;
  private attached = false;
  /** Mouseup listener for copy-on-selection; removed in close(). */
  #copyOnMouseUp: () => void = () => {};
  private cols = 80;
  private rows = 24;
  private syncTimer: ReturnType<typeof setTimeout> | null = null;
  /** Input typed while the socket was connecting; flushed on open. */
  #pendingInput: string[] = [];

  /** Current stream status as a plain value (mirror of the store). */
  private statusValue: SessionStatus = "connecting";

  constructor(prefix: string, gatewayUrl: string) {
    this.prefix = prefix;
    this.gatewayUrl = gatewayUrl.replace(/\/ws\/?$/i, ""); // base for /term/
    // Kick off the WASM load; attach() awaits it before creating the term.
    void ghosttyInit();
    // Shift+drag select → clipboard once on mouse release (no built-in
    // copy-on-selection), with a small toast reporting the outcome.
    // navigator.clipboard only exists in secure contexts (https/localhost);
    // on http:// LAN access, fall back to the legacy execCommand path, which
    // works inside a user gesture.
    this.#copyOnMouseUp = () => {
      const sel = this.term?.getSelection();
      if (!sel) return;
      copyTextToClipboard(sel)
        .then(() => showCopiedToast(true, ""))
        .catch((e) => showCopiedToast(false, String(e)));
    };
    document.addEventListener("mouseup", this.#copyOnMouseUp);
    // Appearance (theme preset / terminal background image) applies live.
    appearance.subscribe(() => this.#applyAppearance());
  }

  /** Attach the terminal to a DOM element and open the stream. `deferred`
   *  (restored-from-layout windows): render the terminal but stay
   *  disconnected — status lands on "ended", whose mask reconnects.
   *  SPIKE: the term is created here (WASM init is async); ghostty-web's
   *  canvas renderer makes the webgl/unicode/image addons unnecessary —
   *  kitty graphics render natively. */
  async attach(el: HTMLElement, deferred = false) {
    if (this.attached) return;
    this.attached = true;
    await ghosttyInit();
    if (!this.term) {
      this.term = new Terminal({
        fontFamily: TERM_FONT_FAMILY,
        fontSize: TERM_FONT_SIZE,
        theme: this.#resolvedTheme(),
        allowTransparency: true,
        cursorBlink: true,
        scrollback: 2000,
      });
      this.fitAddon = new FitAddon();
      this.term.loadAddon(this.fitAddon);
      this.term.onData((data: string) => this.#onData(data));
    }
    this.term.open(el);
    this.fit();
    if (deferred) {
      this.set_status("ended"); // shows the click-to-reconnect mask
      return;
    }
    this.#open();
  }

  #open() {
    const ws = new WebSocket(
      `${this.gatewayUrl}/term/${encodeURIComponent(this.prefix)}?cols=${this.cols}&rows=${this.rows}`,
    );
    ws.binaryType = "arraybuffer";
    this.ws = ws;

    ws.onopen = () => {
      this.set_status("live");
      // Fit now that we know the real container size; the resize text frame
      // makes herdr repaint at the right grid.
      this.fit();
      // Flush input typed while the socket was connecting.
      if (this.#pendingInput.length) {
        const blob = this.#pendingInput.join("");
        this.#pendingInput = [];
        this.#sendInput(blob);
      }
    };
    ws.onmessage = (ev) => {
      if (ev.data instanceof ArrayBuffer) {
        this.term.write(new Uint8Array(ev.data));
      }
      // Text frames carry no stream data (attach failures just close below).
    };
    ws.onclose = () => {
      // A lost gateway and a closed pane both land here; the ⟳ in the window
      // header (reopen) is the way back — no automatic retries.
      if (this.statusValue !== "error") this.set_status("ended");
    };
    ws.onerror = () => this.set_status("error");
  }

  /** Fit the terminal to its container; notifies the gateway on change. */
  fit() {
    if (!this.attached || !this.term) return;
    try {
      this.fitAddon.fit();
    } catch {
      /* container not measurable */
    }
    this.#syncSoon();
  }

  #syncSoon() {
    if (this.syncTimer) clearTimeout(this.syncTimer);
    this.syncTimer = setTimeout(() => {
      this.syncTimer = null;
      const { cols, rows } = this.term;
      if (cols === this.cols && rows === this.rows) return;
      this.cols = cols;
      this.rows = rows;
      this.size.set({ cols, rows });
      this.ws?.send(JSON.stringify({ cols, rows }));
    }, SYNC_DEBOUNCE_MS);
  }

  /** Send raw bytes (keyboard input / keybar), bypassing xterm's onData. */
  sendRaw(data: string) {
    this.#sendInput(data);
  }

  /** Resolve the ghostty theme from the appearance settings: the preset
   *  colors, with a transparent background when a terminal background image
   *  is set. */
  #resolvedTheme() {
    const a = getAppearance();
    const preset = TERM_PRESETS[a.theme] ?? TERM_PRESETS["tokyo-night"];
    const theme = { ...preset.theme };
    if (a.termBg) theme.background = "#00000000";
    return theme;
  }

  #applyAppearance() {
    if (this.term) this.term.options.theme = this.#resolvedTheme();
  }

  /** Scroll the pane via PageUp/PageDown (reaches the running app). */
  scroll(up: boolean) {
    this.#sendInput(up ? "\x1b[5~" : "\x1b[6~");
  }

  /** Scroll by n lines via arrow keys (n > 0 = down, n < 0 = up). */
  scrollLines(n: number) {
    const count = Math.min(Math.abs(Math.round(n)), 10);
    if (count === 0) return;
    this.#sendInput((n < 0 ? "\x1b[A" : "\x1b[B").repeat(count));
  }

  /** Tear down: close the socket (the gateway kills its attach). */
  close() {
    if (this.syncTimer) clearTimeout(this.syncTimer);
    document.removeEventListener("mouseup", this.#copyOnMouseUp);
    if (this.ws) {
      this.ws.onclose = null;
      this.ws.close();
      this.ws = null;
    }
    this.term?.dispose();
  }

  /** Re-open the terminal stream after a disconnect (fresh attach). */
  reopen() {
    if (this.ws) {
      this.ws.onclose = null;
      this.ws.close();
      this.ws = null;
    }
    this.status.set("connecting");
    this.#open();
  }

  /** xterm keyboard output (soft keyboard, real keys, paste): wrap with the
   *  latched sticky modifiers — the mobile keyboard has no Ctrl/Alt — then
   *  clear them. One keypress, one use. */
  #onData(data: string) {
    const m = get(stickyMods);
    if (m.ctrl || m.alt || m.shift) {
      this.#sendInput(applyModsToTyped(data, m));
      clearMods();
    } else {
      this.#sendInput(data);
    }
  }

  #sendInput(data: string) {
    if (this.ws?.readyState !== WebSocket.OPEN) {
      // Still connecting: queue so nothing typed during a reconnect is lost.
      if (this.ws && this.ws.readyState === WebSocket.CONNECTING) {
        this.#pendingInput.push(data);
        if (this.#pendingInput.length > 64) this.#pendingInput.shift();
      }
      return;
    }
    this.ws.send(new TextEncoder().encode(data));
  }
}
