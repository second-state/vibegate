/**
 * Session discovery — now just a thin view over the gateway's pane list.
 *
 * The gateway pushes the authoritative list on connect and on every change,
 * so there is no presence/LWT/staleness logic anymore. Kept as its own module
 * so components have a stable shape (prefix/sid/title/state) to render.
 */

import { derived } from "svelte/store";
import { connection } from "./connection";

export type Instance = {
  /** herdr terminal id — same as the old "prefix" role. */
  sid: string;
  pane_id: string;
  title: string;
  /** Raw herdr agent status (idle/working/blocked/done/unknown), verbatim;
   *  empty for plain shells. */
  state: string;
  agent: string | null;
  /** Raw herdr agent status (same as state, never blanked). */
  agent_status: string;
  cwd: string;
  workspace_id: string;
};

export const discovery = {
  /** All known instances, sorted by title for a stable list. */
  list: derived(connection.panes, (panes) =>
    panes
      .map((p): Instance => ({
        sid: p.sid,
        pane_id: p.pane_id,
        title: p.title || p.sid,
        state: p.agent ? p.agent_status : "",
        agent: p.agent,
        agent_status: p.agent_status,
        cwd: p.cwd,
        workspace_id: p.workspace_id,
      }))
      .sort((a, b) => a.title.localeCompare(b.title, undefined, { numeric: true })),
  ),
};

/** Status → dot color. herdr vocabulary is idle|working|blocked|done|unknown;
 *  anything new herdr adds lands on violet so it stays visible instead of
 *  vanishing into gray. */
export function statusColor(s: string): string {
  switch (s) {
    case "working":
      return "bg-green-400";
    case "blocked":
      return "bg-amber-400";
    case "done":
      return "bg-sky-400";
    case "idle":
    case "":
      return "bg-slate-500";
    default:
      return "bg-violet-400";
  }
}

/** Status → text color (same palette as statusColor). */
export function statusTextCls(s: string): string {
  switch (s) {
    case "working":
      return "text-green-400";
    case "blocked":
      return "text-amber-300";
    case "done":
      return "text-sky-300";
    case "idle":
    case "":
      return "text-slate-500";
    default:
      return "text-violet-300";
  }
}
