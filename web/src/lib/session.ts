/**
 * One terminal session backed by a vibetty instance over MQTT.
 *
 * Wire protocol (truth source: vibetty/src/mqtt.rs):
 * - subscribe `{prefix}/screen_text`: payload = [tag:1B][ANSI bytes],
 *   tag 0x00 = full-screen baseline (reset + replay), 0x01 = pty increment.
 * - publish `{prefix}/pty_in`: raw key bytes.
 * - publish `{prefix}/control`: JSON `{"type":"sync","data":{width,height,pixels,close}}`.
 *   pixels:false → width/height are character cols/rows. close:true pauses
 *   server-side pushing (bandwidth saving when the window is not focused).
 *
 * The screen topics are not retained, so after every (re)subscribe we must
 * send a sync to force a fresh baseline frame.
 */

import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { connection } from "./connection";
import { TERM_FONT_FAMILY, TERM_FONT_SIZE, TERM_THEME } from "./theme";

const SYNC_DEBOUNCE_MS = 300;

export class TerminalSession {
  readonly prefix: string;
  readonly term: Terminal;
  readonly fitAddon: FitAddon;

  /** Whether this session is the focused one (drives the close flag). */
  #active = false;
  /** Dedup key of the last sync we sent: "cols x rows x close". */
  #lastSync: string | null = null;
  #subscribed = false;
  #attached = false;
  #wantClose = false;
  #syncTimer: ReturnType<typeof setTimeout> | null = null;
  #detachMessage: (() => void) | null = null;
  #detachConnect: (() => void) | null = null;
  #webglLoaded = false;

  /** When true, keyboard input is not forwarded (view-only mode). */
  readOnly = false;

  constructor(prefix: string) {
    this.prefix = prefix;
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

    const screenTopic = `${prefix}/screen_text`;
    this.#detachMessage = connection.onMessage((topic, payload) => {
      if (topic === screenTopic) this.#handleScreenText(payload);
    });
    // After a reconnect all subscriptions are gone (clean session) and the
    // terminal may have missed data — resubscribe and force a fresh baseline.
    this.#detachConnect = connection.onConnect(() => {
      this.#subscribeAndSync();
    });
  }

  /** Attach the terminal to a DOM element and start streaming. */
  attach(el: HTMLElement) {
    if (this.#attached) return;
    this.#attached = true;
    this.term.open(el);
    try {
      const webgl = new WebglAddon();
      webgl.onContextLoss(() => webgl.dispose());
      this.term.loadAddon(webgl);
      this.#webglLoaded = true;
    } catch {
      // WebGL unavailable (older browsers / software rendering limits) — the
      // DOM renderer still works.
    }
    this.fit();
    this.#subscribeAndSync();
  }

  /** Focus state changed — tell vibetty to push or pause. */
  setActive(active: boolean) {
    if (this.#active === active) return;
    this.#active = active;
    this.sendSync();
  }

  /** Send raw bytes (e.g. from the keybar), bypassing xterm's onData. */
  sendRaw(data: string) {
    this.#sendInput(data);
  }

  /** Ask the server-side PTY to scroll its scrollback. */
  scroll(up: boolean) {
    const payload = JSON.stringify({
      type: up ? "scroll_up" : "scroll_down",
      data: { rows: 0 },
    });
    connection.publish(`${this.prefix}/control`, payload, 1);
  }

  /** Fit the terminal to its container and sync dimensions to the server. */
  fit() {
    if (!this.#attached) return;
    try {
      this.fitAddon.fit();
    } catch {
      /* container not measurable (hidden / zero-size) */
    }
    this.#syncSoon();
  }

  /** Periodic dimension sync (also used as a keepalive of intent). */
  sendSync(force = false) {
    if (!this.#subscribed && !force) return;
    const close = !this.#active || this.#wantClose;
    const key = `${this.term.cols}x${this.term.rows}x${close}`;
    if (!force && key === this.#lastSync) return;
    this.#lastSync = key;
    const payload = JSON.stringify({
      type: "sync",
      data: {
        width: this.term.cols,
        height: this.term.rows,
        pixels: false,
        close,
      },
    });
    connection.publish(`${this.prefix}/control`, payload, 1);
  }

  /** Tear down: pause pushing, unsubscribe, dispose the terminal. */
  close() {
    this.#wantClose = true;
    this.sendSync(true);
    if (this.#subscribed) connection.unsubscribe(`${this.prefix}/screen_text`);
    this.#subscribed = false;
    this.#detachMessage?.();
    this.#detachConnect?.();
    this.#detachMessage = null;
    this.#detachConnect = null;
    if (this.#syncTimer) clearTimeout(this.#syncTimer);
    this.term.dispose();
  }

  #subscribeAndSync() {
    if (this.#wantClose) return;
    connection.subscribe(`${this.prefix}/screen_text`, 0);
    this.#subscribed = true;
    // Force a baseline: screen topics are not retained, and stale content may
    // be on screen after reconnecting.
    this.term.reset();
    this.#lastSync = null;
    this.sendSync(true);
  }

  #handleScreenText(payload: Uint8Array) {
    if (payload.length < 1) return;
    const tag = payload[0];
    const body = payload.subarray(1);
    if (tag === 0x00) {
      // Full-screen baseline: reset then replay the whole ANSI stream.
      this.term.reset();
      this.term.write(body);
    } else if (tag === 0x01) {
      this.term.write(body);
    }
  }

  #sendInput(data: string) {
    if (this.readOnly || !connection.connected) return;
    connection.publish(`${this.prefix}/pty_in`, new TextEncoder().encode(data), 0);
  }

  #syncSoon() {
    if (this.#syncTimer) clearTimeout(this.#syncTimer);
    this.#syncTimer = setTimeout(() => {
      this.#syncTimer = null;
      this.sendSync();
    }, SYNC_DEBOUNCE_MS);
  }
}
