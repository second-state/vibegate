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
  title: string;
  /** "working" | "waiting" for agents; empty for plain shells. */
  state: string;
  agent: string | null;
  cwd: string;
  workspace_id: string;
};

export const discovery = {
  /** All known instances, sorted by title for a stable list. */
  list: derived(connection.panes, (panes) =>
    panes
      .map((p): Instance => ({
        sid: p.sid,
        title: p.title || p.sid,
        state: mapState(p.agent_status, p.agent),
        agent: p.agent,
        cwd: p.cwd,
        workspace_id: p.workspace_id,
      }))
      .sort((a, b) => a.title.localeCompare(b.title, undefined, { numeric: true })),
  ),
};

/** herdr statuses → the working/waiting pair the UI uses. */
function mapState(status: string, agent: string | null): string {
  if (!agent) return ""; // plain shell: no agent state to show
  switch (status) {
    case "working":
      return "working";
    case "blocked":
    case "idle":
    case "done":
      return "waiting";
    default:
      return "";
  }
}
