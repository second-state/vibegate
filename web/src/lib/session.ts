/**
 * One terminal window: a raw WebSocket to vibegate's `/term/<sid>`.
 *
 * The socket carries pure terminal bytes — vibegate attaches
 * `herdr terminal attach` for us and herdr repaints the full screen on
 * connect/resize, so there is no baseline protocol; bytes are simply appended
 * to xterm in order. Input goes back as binary frames; resize is a small
 * text frame.
 */

import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { writable, type Writable } from "svelte/store";
import { TERM_FONT_FAMILY, TERM_FONT_SIZE, TERM_THEME } from "./theme";

const SYNC_DEBOUNCE_MS = 200;

export type SessionStatus = "connecting" | "live" | "ended" | "error";

export class TerminalSession {
  readonly prefix: string; // kept for compatibility: the herdr terminal id
  readonly term: Terminal;
  readonly fitAddon: FitAddon;

  readonly status: Writable<SessionStatus> = writable("connecting");
  private set_status(v: SessionStatus) { this.statusValue = v; this.status.set(v); }

  private ws: WebSocket | null = null;
  private gatewayUrl: string;
  private attached = false;
  private cols = 80;
  private rows = 24;
  private syncTimer: ReturnType<typeof setTimeout> | null = null;

  /** When true, keyboard input is not forwarded (view-only mode). */
  readOnly = false;

  /** Current stream status as a plain value (mirror of the store). */
  private statusValue: SessionStatus = "connecting";

  constructor(prefix: string, gatewayUrl: string) {
    this.prefix = prefix;
    this.gatewayUrl = gatewayUrl.replace(/\/ws\/?$/i, ""); // base for /term/
    this.term = new Terminal({
      fontFamily: TERM_FONT_FAMILY,
      fontSize: TERM_FONT_SIZE,
      theme: TERM_THEME,
      cursorBlink: true,
      allowProposedApi: true,
      scrollback: 2000,
    });
    this.fitAddon = new FitAddon();
    this.term.loadAddon(this.fitAddon);
    this.term.onData((data) => this.#sendInput(data));
  }

  /** Attach the terminal to a DOM element and open the stream. */
  attach(el: HTMLElement) {
    if (this.attached) return;
    this.attached = true;
    this.term.open(el);
    try {
      const webgl = new WebglAddon();
      webgl.onContextLoss(() => webgl.dispose());
      this.term.loadAddon(webgl);
    } catch {
      // WebGL unavailable — the DOM renderer still works.
    }
    this.fit();
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
    };
    ws.onmessage = (ev) => {
      if (ev.data instanceof ArrayBuffer) {
        this.term.write(new Uint8Array(ev.data));
      } else if (typeof ev.data === "string") {
        // Only text we accept is the resize echo path; nothing to render.
      }
    };
    ws.onclose = () => {
      if (this.statusValue !== "error") this.set_status("ended");
    };
    ws.onerror = () => this.set_status("error");
  }

  /** Fit the terminal to its container; notifies the gateway on change. */
  fit() {
    if (!this.attached) return;
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
      this.ws?.send(JSON.stringify({ cols, rows }));
    }, SYNC_DEBOUNCE_MS);
  }

  /** Send raw bytes (keyboard input / keybar), bypassing xterm's onData. */
  sendRaw(data: string) {
    this.#sendInput(data);
  }

  /** Scroll the pane via PageUp/PageDown (reaches the running app). */
  scroll(up: boolean) {
    this.#sendInput(up ? "\x1b[5~" : "\x1b[6~");
  }

  /** Tear down: close the socket (the gateway kills its attach). */
  close() {
    if (this.syncTimer) clearTimeout(this.syncTimer);
    if (this.ws) {
      this.ws.onclose = null;
      this.ws.close();
      this.ws = null;
    }
    this.term.dispose();
  }

  #sendInput(data: string) {
    if (this.readOnly) return;
    if (this.ws?.readyState !== WebSocket.OPEN) return;
    this.ws.send(new TextEncoder().encode(data));
  }
}
