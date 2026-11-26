//! herdr-core (`hc`) backend — session feed.
//!
//! Spawns `hc events` as a long-running child and parses its stdout (piped,
//! so the CLI emits JSON lines, one per message):
//! - line 1 is the subscription snapshot:
//!   `{"channel":null,"seq":N,"terminals":[Terminal...]}` (full records with
//!   labels/status/foreground);
//! - every following line is a JSON-RPC notification: `event` (params:
//!   `{seq,type,terminal,data}`), `heartbeat`, or `missed` (params is a fresh
//!   snapshot — drop everything and reset).
//!
//! Key events go out on an mpsc channel; the consumer in `main` keeps the
//! terminal map and rebuilds the `{"type":"sessions",...}` payload for the
//! websocket clients. No socket path is needed: both the feed and the
//! per-pane attach (`hc attach <t>`) run through the `hc` CLI, which
//! resolves the core socket itself.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::Command;
use tokio::sync::mpsc;

/// Plain-mode JSON-RPC client for the local core socket (one-shot calls:
/// connect, one line each way — same pattern as the herdr client). Used for
/// the workspace control ops; the attach/feed paths go through the `hc` CLI.
pub struct HcClient {
    socket: PathBuf,
}

static REQ_ID: AtomicU64 = AtomicU64::new(1);

impl HcClient {
    pub fn new(socket: impl Into<PathBuf>) -> Self {
        Self {
            socket: socket.into(),
        }
    }

    /// Resolve the local core's client socket, per docs/security.md:
    /// `HC_SOCKET` env wins; else `$XDG_RUNTIME_DIR/herdr-core/<session>.sock`,
    /// else `/run/user/<uid>/herdr-core/` if it exists, else the session's
    /// state directory. The session name is "default" unless `HC_SESSION`
    /// is set.
    pub fn default_socket() -> PathBuf {
        if let Ok(p) = std::env::var("HC_SOCKET") {
            return PathBuf::from(p);
        }
        let session = std::env::var("HC_SESSION").unwrap_or_else(|_| "default".to_string());
        let name = format!("{session}.sock");
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
            candidates.push(PathBuf::from(dir).join("herdr-core").join(&name));
        }
        if let Some(uid) = current_uid() {
            candidates.push(
                PathBuf::from("/run/user")
                    .join(uid.to_string())
                    .join("herdr-core")
                    .join(&name),
            );
        }
        let state_home = std::env::var("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_default();
                PathBuf::from(home).join(".local").join("state")
            });
        candidates.push(state_home.join("herdr-core").join(&session).join(&name));
        candidates
            .into_iter()
            .find(|p| p.exists())
            .unwrap_or_else(|| PathBuf::from(name)) // last resort: bare name
    }

    /// One-shot JSON-RPC call over the socket; returns the `result` value.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let stream = UnixStream::connect(&self.socket)
            .await
            .with_context(|| format!("connecting to hc socket {}", self.socket.display()))?;
        let id = REQ_ID.fetch_add(1, Ordering::Relaxed);
        let req = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let (rd, mut wr) = stream.into_split();
        wr.write_all(serde_json::to_string(&req)?.as_bytes())
            .await?;
        wr.write_all(b"\n").await?;
        wr.shutdown().await?;

        let mut line = String::new();
        BufReader::new(rd).read_line(&mut line).await?;
        if line.is_empty() {
            bail!("hc closed the connection without replying ({method})");
        }
        let v: Value = serde_json::from_str(line.trim())?;
        if let Some(err) = v.get("error") {
            bail!("hc {method} error: {err}");
        }
        Ok(v.get("result").cloned().unwrap_or(Value::Null))
    }
}

/// Read the real uid out of /proc/self/status (UID is not always exported).
fn current_uid() -> Option<String> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("Uid:") {
            return rest.split_whitespace().next().map(String::from);
        }
    }
    None
}

/// One terminal, narrowed to what vibegate's session list needs.
#[derive(Debug, Clone)]
pub struct HcTerminal {
    /// Core address: `t7` locally, `machine/t7` for remote machines.
    pub address: String,
    pub title: String,
    /// `live` / `finished` / ... (terminal record state; used by future
    /// filtering, not the sessions payload yet).
    #[allow(dead_code)]
    pub state: String,
    /// Status reported via OSC 7501: `app` (agent brand) and `state`
    /// (working/done/blocked/error/... — reported by the program itself).
    pub agent: Option<String>,
    pub agent_status: Option<String>,
    /// Foreground process cwd (core re-reads /proc ~2x/s), else spawn cwd.
    pub cwd: String,
    /// vibegate's workspace grouping rides on the `workspace` label
    /// (labels are free-form key=value managed by clients).
    pub workspace: String,
    pub labels: BTreeMap<String, String>,
}

/// The subset of core events vibegate cares about.
#[derive(Debug, Clone)]
pub enum HcEvent {
    /// Full reset: subscribe snapshot or a `missed` notification.
    Snapshot(Vec<HcTerminal>),
    /// `created` / `restarted` — the whole record.
    Upsert(HcTerminal),
    /// `removed` — drop the terminal.
    Remove(String),
    /// `status` — root status replaced (may be None = cleared).
    Status {
        terminal: String,
        app: Option<String>,
        state: Option<String>,
    },
    /// `title`.
    Title { terminal: String, title: String },
    /// `labels` — the complete label map now.
    Labels {
        terminal: String,
        labels: BTreeMap<String, String>,
    },
    /// `foreground` — process group leader changed (cwd rides along).
    Foreground {
        terminal: String,
        cwd: Option<String>,
    },
    /// `finished` / `lost` — the program ended or was lost.
    Ended { terminal: String },
}

/// Spawn `hc events` and feed parsed events forever; respawns after 2 s if
/// the child dies (core restart, upgrade). Returns only if the consumer
/// hangs up.
pub async fn run_feed(tx: mpsc::Sender<HcEvent>) -> Result<()> {
    loop {
        let mut child = Command::new("hc")
            .arg("events")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("spawning `hc events`")?;
        let Some(stdout) = child.stdout.take() else {
            bail!("hc events produced no stdout");
        };
        let mut lines = BufReader::new(stdout).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    for ev in parse_line(&line) {
                        if tx.send(ev).await.is_err() {
                            let _ = child.kill().await;
                            let _ = child.wait().await;
                            return Ok(()); // consumer gone
                        }
                    }
                }
                Ok(None) => break, // child closed stdout
                Err(e) => {
                    tracing::warn!("hc events read error: {e}");
                    break;
                }
            }
        }
        let _ = child.kill().await;
        let _ = child.wait().await; // reap, no zombie
        tracing::warn!("hc events exited; respawning in 2s");
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

/// Parse one stdout line into zero or more events.
pub fn parse_line(line: &str) -> Vec<HcEvent> {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Vec::new();
    };
    // The subscribe reply and `missed` params both carry a `terminals`
    // array: treat either as a full reset.
    if let Some(list) = v.get("terminals").and_then(Value::as_array) {
        return vec![HcEvent::Snapshot(terminals_from(list))];
    }
    match v.get("method").and_then(Value::as_str) {
        Some("heartbeat") => Vec::new(),
        Some("missed") => parse_missed(v.get("params")),
        Some("event") => parse_event(v.get("params")),
        _ => Vec::new(),
    }
}

fn parse_missed(params: Option<&Value>) -> Vec<HcEvent> {
    match params
        .and_then(|p| p.get("terminals"))
        .and_then(Value::as_array)
    {
        Some(list) => vec![HcEvent::Snapshot(terminals_from(list))],
        None => Vec::new(),
    }
}

fn parse_event(params: Option<&Value>) -> Vec<HcEvent> {
    let Some(p) = params else { return Vec::new() };
    let terminal = str_of(p, "terminal").unwrap_or_default();
    let data = p.get("data");
    match str_of(p, "type").as_deref() {
        Some("created") | Some("restarted") => data
            .and_then(terminal_from_value)
            .map(|t| vec![HcEvent::Upsert(t)])
            .unwrap_or_default(),
        Some("removed") => vec![HcEvent::Remove(terminal)],
        Some("status") => {
            let status = data.and_then(|d| d.get("status"));
            vec![HcEvent::Status {
                terminal,
                app: status.and_then(|s| str_of(s, "app")),
                state: status.and_then(|s| str_of(s, "state")),
            }]
        }
        Some("title") => vec![HcEvent::Title {
            terminal,
            title: data.and_then(|d| str_of(d, "title")).unwrap_or_default(),
        }],
        Some("labels") => vec![HcEvent::Labels {
            terminal,
            // data is `{labels: {...}}` (plus an optional `record`)
            labels: labels_from(data.and_then(|d| d.get("labels"))),
        }],
        Some("foreground") => vec![HcEvent::Foreground {
            terminal,
            cwd: data.and_then(|d| str_of(d, "cwd")),
        }],
        Some("finished") | Some("lost") => vec![HcEvent::Ended { terminal }],
        // keep / turn / size / doc: not interesting to the gateway.
        _ => Vec::new(),
    }
}

fn terminals_from(list: &[Value]) -> Vec<HcTerminal> {
    list.iter()
        // Core snapshots keep recently finished terminals around; herdr's
        // snapshot only ever exposed live panes, so drop finished here.
        .filter(|v| v.get("state").and_then(Value::as_str) != Some("finished"))
        .filter_map(terminal_from_value)
        .collect()
}

fn terminal_from_value(v: &Value) -> Option<HcTerminal> {
    let address = str_of(v, "address")?;
    let labels = labels_from(v.get("labels"));
    let status = v.get("status");
    let foreground_cwd = v.get("foreground").and_then(|f| str_of(f, "cwd"));
    Some(HcTerminal {
        workspace: labels.get("workspace").cloned().unwrap_or_default(),
        labels,
        address,
        title: str_of(v, "title").unwrap_or_default(),
        state: str_of(v, "state").unwrap_or_default(),
        agent: status.and_then(|s| str_of(s, "app")),
        agent_status: status.and_then(|s| str_of(s, "state")),
        cwd: foreground_cwd
            .or_else(|| str_of(v, "cwd"))
            .unwrap_or_default(),
    })
}

fn labels_from(v: Option<&Value>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Some(obj) = v.and_then(Value::as_object) {
        for (k, val) in obj {
            if let Some(s) = val.as_str() {
                out.insert(k.clone(), s.to_string());
            }
        }
    }
    out
}

fn str_of(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_reply_line() {
        // First line of `hc events` (piped): the subscribe reply.
        let line = r#"{"channel":null,"seq":58,"terminals":[
            {"address":"t6","argv":["zsh"],"cols":200,"created_ms":1795697796384,
             "cwd":"/home/csh/Work","die_with_parent":false,"exit_code":null,
             "finished_ms":null,"foreground":{"argv":["pi"],"cwd":"/home/csh/Work",
             "name":"pi","pid":330450},"identity":"ghostty","keep":false,
             "labels":{"workspace":"p"},"name":null,"notify":true,"parent":null,
             "pid":328505,"reason":null,"reported":true,"rm":false,"rows":120,
             "screen":"exact","signal":null,"state":"live",
             "status":{"app":"pi","state":"done"},
             "status_ms":1795699226583,"template":null,"title":"π - Work","turn":1}
        ]}"#;
        let evs = parse_line(line);
        assert_eq!(evs.len(), 1);
        match &evs[0] {
            HcEvent::Snapshot(list) => {
                assert_eq!(list.len(), 1);
                let t = &list[0];
                assert_eq!(t.address, "t6");
                assert_eq!(t.title, "π - Work");
                assert_eq!(t.state, "live");
                assert_eq!(t.agent.as_deref(), Some("pi"));
                assert_eq!(t.agent_status.as_deref(), Some("done"));
                assert_eq!(t.cwd, "/home/csh/Work"); // foreground cwd wins
                assert_eq!(t.workspace, "p");
            }
            other => panic!("expected Snapshot, got {other:?}"),
        }
    }

    #[test]
    fn status_event() {
        let line = r#"{"jsonrpc":"2.0","method":"event","params":{"seq":59,
            "type":"status","terminal":"t7",
            "data":{"status":{"app":"claude","state":"working"},"records":{},"reported":true}}}"#;
        match &parse_line(line)[0] {
            HcEvent::Status {
                terminal,
                app,
                state,
            } => {
                assert_eq!(terminal, "t7");
                assert_eq!(app.as_deref(), Some("claude"));
                assert_eq!(state.as_deref(), Some("working"));
            }
            other => panic!("expected Status, got {other:?}"),
        }
    }

    #[test]
    fn created_event() {
        let line = r#"{"jsonrpc":"2.0","method":"event","params":{"seq":60,
            "type":"created","terminal":"t8",
            "data":{"address":"t8","argv":["zsh"],"cols":80,"created_ms":1,
            "cwd":"/tmp","state":"live","rows":24,"labels":{},"title":null}}}"#;
        match &parse_line(line)[0] {
            HcEvent::Upsert(t) => {
                assert_eq!(t.address, "t8");
                assert_eq!(t.cwd, "/tmp");
                assert_eq!(t.workspace, "");
                assert!(t.agent.is_none());
            }
            other => panic!("expected Upsert, got {other:?}"),
        }
    }

    #[test]
    fn labels_foreground_removed_events() {
        let labels = parse_line(
            r#"{"jsonrpc":"2.0","method":"event","params":{"seq":1,"type":"labels",
               "terminal":"t6","data":{"labels":{"workspace":"dev"}}}}"#,
        );
        match &labels[0] {
            HcEvent::Labels { terminal, labels } => {
                assert_eq!(terminal, "t6");
                assert_eq!(labels.get("workspace").map(String::as_str), Some("dev"));
            }
            other => panic!("expected Labels, got {other:?}"),
        }
        let fg = parse_line(
            r#"{"jsonrpc":"2.0","method":"event","params":{"seq":2,"type":"foreground",
               "terminal":"t6","data":{"pid":9,"name":"vim","argv":["vim"],
               "cwd":"/home/x/proj"}}}"#,
        );
        match &fg[0] {
            HcEvent::Foreground { terminal, cwd } => {
                assert_eq!(terminal, "t6");
                assert_eq!(cwd.as_deref(), Some("/home/x/proj"));
            }
            other => panic!("expected Foreground, got {other:?}"),
        }
        let rm = parse_line(
            r#"{"jsonrpc":"2.0","method":"event","params":{"seq":3,"type":"removed",
               "terminal":"t9","data":{}}}"#,
        );
        assert!(matches!(&rm[0], HcEvent::Remove(t) if t == "t9"));
    }

    #[test]
    fn heartbeat_ignored_and_missed_resets() {
        assert!(
            parse_line(r#"{"jsonrpc":"2.0","method":"heartbeat","params":{"seq":42}}"#).is_empty()
        );
        let missed = parse_line(
            r#"{"jsonrpc":"2.0","method":"missed","params":{"seq":57,
               "terminals":[{"address":"t1","labels":{},"argv":["sh"],"cwd":"/",
               "state":"live","cols":80,"rows":24}]}}"#,
        );
        assert!(matches!(&missed[0], HcEvent::Snapshot(list) if list.len() == 1));
    }

    #[test]
    fn garbage_line_is_ignored() {
        assert!(parse_line("not json at all").is_empty());
    }
}
