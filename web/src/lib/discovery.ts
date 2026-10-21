/**
 * vibetty instance discovery via retained presence announcements.
 *
 * Each vibetty instance publishes presence on its own topic prefix
 * `{user}/{device}/{pid}/vibetty` (retained, 15s heartbeat, LWT clears it).
 * We subscribe `+/+/+/vibetty` and maintain a live instance list:
 * empty payload = offline (LWT); stale `ts` (> 30s) = offline fallback.
 */

import { derived, writable, type Writable } from "svelte/store";
import { connection } from "./connection";

export type Presence = {
  /** Full instance topic prefix, doubles as the map key. */
  prefix: string;
  clientId: string;
  /** Epoch seconds of the last heartbeat. */
  ts: number;
  /** Terminal window title (e.g. "claude — workspace"). */
  title: string;
  /** Agent state: "working" | "waiting". */
  state: string;
  /** Output format: "text" expected; anything else is unsupported here. */
  format: string;
};

/** Consider an instance offline if its heartbeat is older than this. */
const STALE_SECS = 30;
const SWEEP_INTERVAL_MS = 5000;

class Discovery {
  readonly instances: Writable<Map<string, Presence>> = writable(new Map());

  /** Instances sorted by name — heartbeats refresh `ts` every 15s, so sorting
   *  by it would make the list jump around; a name order stays stable. */
  readonly list = derived(this.instances, (map) =>
    [...map.values()].sort((a, b) => {
      const an = a.title || a.prefix;
      const bn = b.title || b.prefix;
      return (
        an.localeCompare(bn, undefined, { numeric: true, sensitivity: "base" }) ||
        a.prefix.localeCompare(b.prefix)
      );
    }),
  );

  #map = new Map<string, Presence>();
  #sweepTimer: ReturnType<typeof setInterval> | null = null;
  #detachFns: (() => void)[] = [];

  attach() {
    if (this.#detachFns.length > 0) return;
    this.#detachFns.push(connection.onMessage((t, p) => this.#handle(t, p)));
    this.#detachFns.push(
      connection.onConnect(() => connection.subscribe("+/+/+/vibetty", 1)),
    );
    this.#sweepTimer = setInterval(() => this.#sweepStale(), SWEEP_INTERVAL_MS);
  }

  detach() {
    for (const fn of this.#detachFns) fn();
    this.#detachFns = [];
    if (this.#sweepTimer) {
      clearInterval(this.#sweepTimer);
      this.#sweepTimer = null;
    }
  }

  getInstance(prefix: string): Presence | undefined {
    return this.#map.get(prefix);
  }

  #publish() {
    this.instances.set(new Map(this.#map));
  }

  #handle(topic: string, payload: Uint8Array) {
    const segs = topic.split("/");
    if (segs.length !== 4 || segs[3] !== "vibetty") return;

    if (payload.length === 0) {
      // LWT: empty retained payload = instance went offline.
      if (this.#map.has(topic)) {
        this.#map.delete(topic);
        this.#publish();
      }
      return;
    }

    let p: any;
    try {
      p = JSON.parse(new TextDecoder().decode(payload));
    } catch {
      return;
    }
    if (!p || typeof p !== "object") return;

    this.#map.set(topic, {
      prefix: typeof p.prefix === "string" ? p.prefix : topic,
      clientId: String(p.client_id ?? ""),
      ts: Number(p.ts ?? 0),
      title: String(p.title ?? ""),
      state: String(p.state ?? ""),
      format: String(p.format ?? "text"),
    });
    this.#publish();
  }

  #sweepStale() {
    const now = Date.now() / 1000;
    let changed = false;
    for (const [key, ins] of this.#map) {
      if (ins.ts > 0 && now - ins.ts > STALE_SECS) {
        this.#map.delete(key);
        changed = true;
      }
    }
    if (changed) this.#publish();
  }
}

export const discovery = new Discovery();
