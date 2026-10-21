/**
 * MQTT-over-WebSocket connection management.
 *
 * A single shared client connects to the broker; message routing is done by
 * registered listeners (discovery, terminal sessions). Follows the wire
 * behavior of vibetty's `/mqtt_ws` debug page.
 */

import mqtt, { type MqttClient } from "mqtt";
import { writable, type Writable } from "svelte/store";

export type ConnState =
  | "idle"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "error";

const URL_KEY = "vibeview.brokerUrl";
const USER_KEY = "vibeview.brokerUser";
const PASS_KEY = "vibeview.brokerPass";

const safeGet = (k: string): string | null => {
  try {
    return localStorage.getItem(k);
  } catch {
    return null;
  }
};
const safeSet = (k: string, v: string) => {
  try {
    localStorage.setItem(k, v);
  } catch {
    /* private mode */
  }
};
const safeDel = (k: string) => {
  try {
    localStorage.removeItem(k);
  } catch {
    /* ignore */
  }
};

/** Saved connection settings, with `?broker=`/`?url=` query param override. */
export function loadSettings(): {
  url: string;
  username: string;
  password: string;
} {
  const q = new URLSearchParams(window.location.search);
  const url =
    q.get("broker") || q.get("url") || safeGet(URL_KEY) || "ws://localhost:9001/mqtt";
  return {
    url,
    username: safeGet(USER_KEY) ?? "",
    password: safeGet(PASS_KEY) ?? "",
  };
}

export function saveSettings(url: string, username: string, password: string) {
  if (url) safeSet(URL_KEY, url);
  else safeDel(URL_KEY);
  if (username) safeSet(USER_KEY, username);
  else safeDel(USER_KEY);
  if (password) safeSet(PASS_KEY, password);
  else safeDel(PASS_KEY);
}

/** Returns an error message, or null if the URL looks valid. */
export function validateBrokerUrl(url: string): string | null {
  if (!/^wss?:\/\//i.test(url)) return "URL 须 ws:// 或 wss:// 开头";
  try {
    const u = new URL(url);
    if (!u.pathname.endsWith("/mqtt")) {
      return "URL 路径必须以 /mqtt 结尾(如 ws://host:9001/mqtt)";
    }
    return null;
  } catch {
    return "URL 格式无效";
  }
}

/** https pages cannot connect to ws:// (mixed content is blocked by browsers). */
export function mixedContentWarning(url: string): string | null {
  if (window.location.protocol === "https:" && url.startsWith("ws://")) {
    return "当前页面是 https,浏览器会拦截 ws:// 明文连接——请改用 wss://,或把本页部署在 http 下";
  }
  return null;
}

type MessageListener = (topic: string, payload: Uint8Array) => void;
type ConnectListener = () => void;

class Connection {
  readonly state: Writable<ConnState> = writable("idle");
  readonly error: Writable<string | null> = writable(null);

  client: MqttClient | null = null;

  #messageListeners = new Set<MessageListener>();
  #connectListeners = new Set<ConnectListener>();

  get connected(): boolean {
    return this.client?.connected ?? false;
  }

  onMessage(listener: MessageListener): () => void {
    this.#messageListeners.add(listener);
    return () => this.#messageListeners.delete(listener);
  }

  /** Called on every (re)connect — use to (re)subscribe topics. */
  onConnect(listener: ConnectListener): () => void {
    this.#connectListeners.add(listener);
    return () => this.#connectListeners.delete(listener);
  }

  connect(url: string, username: string, password: string) {
    this.disconnect();
    this.error.set(null);

    const clientId = `vibeview-${Math.random().toString(16).slice(2, 10)}`;
    let client: MqttClient;
    try {
      client = mqtt.connect(url, {
        clientId,
        clean: true,
        keepalive: 30,
        reconnectPeriod: 3000,
        connectTimeout: 8000,
        username: username || undefined,
        password: password || undefined,
      });
    } catch (e) {
      this.state.set("error");
      this.error.set(`连接异常: ${e}`);
      return;
    }
    this.client = client;
    this.state.set("connecting");

    client.on("connect", () => {
      this.state.set("connected");
      this.error.set(null);
      for (const fn of this.#connectListeners) {
        try {
          fn();
        } catch (err) {
          console.error("connect listener failed", err);
        }
      }
    });
    client.on("reconnect", () => {
      this.state.set("reconnecting");
    });
    client.on("close", () => {
      if (!client.connected) this.state.set("reconnecting");
    });
    client.on("offline", () => {
      this.state.set("reconnecting");
    });
    client.on("error", (e) => {
      const msg =
        e && typeof e === "object" && "message" in e ? e.message : String(e);
      this.error.set(msg);
      // auth/protocol errors are fatal — mqtt.js would loop forever otherwise
      if (client.connected) return;
      this.state.set("error");
      client.end(true);
      this.client = null;
    });
    client.on("message", (topic, payload) => {
      const bytes = new Uint8Array(payload);
      for (const fn of this.#messageListeners) {
        try {
          fn(topic, bytes);
        } catch (err) {
          console.error("message listener failed", err);
        }
      }
    });
  }

  disconnect() {
    if (this.client) {
      this.client.end(true);
      this.client = null;
    }
    this.state.set("idle");
  }

  publish(topic: string, payload: Uint8Array | string, qos: 0 | 1 = 0) {
    // Pass Uint8Array straight through: mqtt.js's browser bundle has its own
    // buffer shim (the Node global `Buffer` does not exist in Vite builds).
    this.client?.publish(topic, payload as any, { qos, retain: false });
  }

  subscribe(topic: string, qos: 0 | 1 = 0) {
    this.client?.subscribe(topic, { qos });
  }

  unsubscribe(topic: string) {
    this.client?.unsubscribe(topic);
  }
}

export const connection = new Connection();
