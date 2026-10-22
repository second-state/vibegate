/**
 * vibegate gateway connection: one control WebSocket per page.
 *
 * Replaces the old MQTT transport. The server (vibegate) pushes the pane list
 * on connect and whenever it changes; terminal bytes flow on separate
 * `/term/<sid>` sockets owned by each TerminalSession.
 */

import { writable, type Writable } from "svelte/store";

export type ConnState =
  | "idle"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "error";

export type PaneInfo = {
  /** herdr terminal id — the address of everything (term socket, create). */
  sid: string;
  pane_id: string;
  title: string;
  agent: string | null;
  agent_status: string;
  cwd: string;
  workspace_id: string;
};

export type WorkspaceInfo = {
  id: string;
  label: string;
  number: number;
  pane_count: number;
};

/**
 * Default gateway URL: same origin as this page (`ws(s)://host/ws`), since
 * vibegate serves the frontend and the WebSocket side by side. An explicit
 * `?gateway=`/`?broker=`/`?url=` query parameter overrides it — useful when
 * the frontend is hosted elsewhere.
 */
export function loadGatewayUrl(): string {
  const q = new URLSearchParams(window.location.search);
  const explicit = q.get("gateway") || q.get("broker") || q.get("url");
  if (explicit) return explicit;
  if (!window.location.host) return "ws://localhost:8480/ws"; // file:// edge case
  const proto = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${window.location.host}/ws`;
}

/** Returns an error message, or null if the URL looks valid. */
export function validateGatewayUrl(url: string): string | null {
  if (!/^wss?:\/\//i.test(url)) return "URL 须 ws:// 或 wss:// 开头";
  try {
    new URL(url);
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

class Connection {
  readonly state: Writable<ConnState> = writable("idle");
  readonly error: Writable<string | null> = writable(null);
  /** Latest pane list pushed by the gateway. */
  readonly panes: Writable<PaneInfo[]> = writable([]);
  /** Latest workspace list pushed by the gateway. */
  readonly workspaces: Writable<WorkspaceInfo[]> = writable([]);

  private ws: WebSocket | null = null;
  private url = "";
  private disposed = false;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  get connected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  /** The URL the current connection (or last attempt) used. */
  get urlValue(): string {
    return this.url;
  }

  connect(url: string) {
    this.disconnect();
    this.error.set(null);
    this.url = url;
    this.disposed = false;
    this.state.set("connecting");

    this.open();
  }

  private open() {
    let ws: WebSocket;
    try {
      ws = new WebSocket(this.url);
      ws.binaryType = "arraybuffer";
    } catch (e) {
      this.state.set("error");
      this.error.set(`连接异常: ${e}`);
      return;
    }
    this.ws = ws;

    ws.onopen = () => {
      this.state.set("connected");
      this.error.set(null);
    };
    ws.onmessage = (ev) => {
      let msg: any;
      try {
        msg = JSON.parse(ev.data as string);
      } catch {
        return;
      }
      if (msg.type === "sessions" && Array.isArray(msg.panes)) {
        this.panes.set(msg.panes as PaneInfo[]);
        if (Array.isArray(msg.workspaces)) {
          this.workspaces.set(msg.workspaces as WorkspaceInfo[]);
        }
      } else if (msg.type === "error") {
        this.error.set(String(msg.message ?? "gateway error"));
      }
    };
    ws.onclose = () => {
      if (this.disposed) return;
      this.state.set("reconnecting");
      // The gateway is a single binary; a short retry covers restarts.
      this.reconnectTimer = setTimeout(() => this.open(), 3000);
    };
    ws.onerror = () => {
      if (!this.connected) {
        this.state.set("reconnecting");
      }
    };
  }

  disconnect() {
    this.disposed = true;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.ws) {
      this.ws.onclose = null;
      this.ws.close();
      this.ws = null;
    }
    this.panes.set([]);
    this.workspaces.set([]);
    this.state.set("idle");
  }

  send(msg: object) {
    if (this.connected && this.ws) {
      this.ws.send(JSON.stringify(msg));
    }
  }

  /** Generic create: sends `kind`, waits for the matching reply type. */
  #create(
    kind: "create" | "create_pane",
    payload: Record<string, unknown>,
    onCreated: (sid: string, workspaceId: string | null) => void,
  ) {
    if (!this.connected || !this.ws) return;
    const ws = this.ws;
    const expect = kind === "create" ? "created" : "created_pane";
    const handler = (ev: MessageEvent) => {
      let m: any;
      try {
        m = JSON.parse(ev.data as string);
      } catch {
        return;
      }
      if (m.type === expect) {
        ws.removeEventListener("message", handler);
        onCreated(m.sid as string, (m.workspace_id as string) ?? null);
      }
    };
    ws.addEventListener("message", handler);
    this.send({ type: kind, ...payload });
  }

  /** Create a new workspace with a fresh shell; `created` arrives async. */
  createWorkspace(
    label: string,
    cwd: string | undefined,
    onCreated: (sid: string, workspaceId: string | null) => void = () => {},
  ) {
    this.#create("create", { label, cwd }, onCreated);
  }

  /** Create a fresh shell pane inside an existing workspace. */
  createPane(
    workspaceId: string,
    cwd: string | undefined,
    onCreated: (sid: string, workspaceId: string | null) => void = () => {},
  ) {
    this.#create("create_pane", { workspace_id: workspaceId, cwd }, onCreated);
  }

  /** Close a whole workspace (all its panes die). */
  killWorkspace(workspaceId: string) {
    this.send({ type: "kill", workspace_id: workspaceId });
  }

  /** Close a single pane. */
  killPane(paneId: string) {
    this.send({ type: "kill_pane", pane_id: paneId });
  }
}

export const connection = new Connection();
