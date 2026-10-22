//! herdr socket API client.
//!
//! Transport facts (empirically documented in Collie's HERDR_API.md, confirmed
//! against herdr 0.9.3 / protocol 22 on this machine):
//! - Unix socket (default `~/.config/herdr/herdr.sock`), newline-delimited JSON.
//! - One request per connection; the server closes after answering.
//! - Request `{"id":"<string>","method":...,"params":{...}}`, response
//!   `{"id":...,"result":{...}}` or `{"id":"","error":{"code","message"}}`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

static REQ_ID: AtomicU64 = AtomicU64::new(1);

pub struct Herdr {
    socket: PathBuf,
}

/// One pane from `session.snapshot`, narrowed to what vibegate needs.
#[derive(Debug, Clone)]
pub struct PaneInfo {
    pub pane_id: String,
    pub terminal_id: String,
    pub title: String,
    pub agent: Option<String>,
    pub agent_status: String,
    pub cwd: String,
    pub workspace_id: String,
}

impl Herdr {
    pub fn new(socket: impl Into<PathBuf>) -> Self {
        Self { socket: socket.into() }
    }

    pub fn default_socket() -> PathBuf {
        if let Ok(p) = std::env::var("HERDR_SOCKET_PATH") {
            return PathBuf::from(p);
        }
        let config_root = std::env::var("HERDR_CONFIG_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs_or_home().join(".config").join("herdr")
            });
        config_root.join("herdr.sock")
    }

    /// One-shot RPC: connect, send one line, read one reply line.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let stream = UnixStream::connect(&self.socket)
            .await
            .with_context(|| format!("connecting to herdr socket {}", self.socket.display()))?;
        let id = format!("vibegate-{}", REQ_ID.fetch_add(1, Ordering::Relaxed));
        let req = json!({"id": id, "method": method, "params": params});
        let (rd, mut wr) = stream.into_split();
        wr.write_all(serde_json::to_string(&req)?.as_bytes()).await?;
        wr.write_all(b"\n").await?;
        wr.shutdown().await?;

        let mut line = String::new();
        BufReader::new(rd).read_line(&mut line).await?;
        if line.is_empty() {
            bail!("herdr closed the connection without replying ({method})");
        }
        let v: Value = serde_json::from_str(line.trim())?;
        if let Some(err) = v.get("error") {
            bail!("herdr {method} error: {err}");
        }
        Ok(v.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Full `session.snapshot` result (`snapshot` object).
    pub async fn snapshot(&self) -> Result<Value> {
        let result = self.call("session.snapshot", json!({})).await?;
        result
            .get("snapshot")
            .cloned()
            .context("session_snapshot missing `snapshot`")
    }

    /// `session.snapshot` narrowed to panes.
    pub async fn panes(&self) -> Result<Vec<PaneInfo>> {
        let snapshot = self.snapshot().await?;
        Ok(panes_from_snapshot(&snapshot))
    }

    /// Create a fresh shell tab (and its pane) inside an existing workspace.
    pub async fn create_tab(&self, workspace_id: &str, cwd: Option<&str>) -> Result<Value> {
        self.call(
            "tab.create",
            json!({"workspace_id": workspace_id, "cwd": cwd, "focus": false}),
        )
        .await
    }

    /// Close a single pane (herdr kills its process group).
    pub async fn close_pane(&self, pane_id: &str) -> Result<Value> {
        self.call("pane.close", json!({"pane_id": pane_id})).await
    }

    /// Create a workspace with a fresh shell pane; returns the new workspace id.
    pub async fn create_workspace(&self, label: &str, cwd: Option<&str>) -> Result<Value> {
        self.call(
            "workspace.create",
            json!({"label": label, "cwd": cwd, "focus": false}),
        )
        .await
    }

    pub async fn close_workspace(&self, workspace_id: &str) -> Result<Value> {
        self.call("workspace.close", json!({"workspace_id": workspace_id}))
            .await
    }
}

fn str_of(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(String::from)
}

/// Extract panes from a snapshot object.
pub fn panes_from_snapshot(snapshot: &Value) -> Vec<PaneInfo> {
    let Some(panes) = snapshot.get("panes").and_then(Value::as_array) else {
        return Vec::new();
    };
    panes
        .iter()
        .filter_map(|p| {
            Some(PaneInfo {
                pane_id: str_of(p, "pane_id")?,
                terminal_id: str_of(p, "terminal_id")?,
                title: str_of(p, "terminal_title_stripped").unwrap_or_default(),
                agent: p.get("agent").and_then(Value::as_str).map(String::from),
                agent_status: str_of(p, "agent_status").unwrap_or_else(|| "unknown".into()),
                cwd: str_of(p, "cwd").unwrap_or_default(),
                workspace_id: str_of(p, "workspace_id").unwrap_or_default(),
            })
        })
        .collect()
}

/// One workspace from a snapshot, narrowed for the drawer tree.
#[derive(Debug, Clone)]
pub struct WorkspaceInfo {
    pub workspace_id: String,
    pub label: String,
    pub number: u64,
    pub pane_count: u64,
}

/// Extract workspaces from a snapshot object, ordered by number.
pub fn workspaces_from_snapshot(snapshot: &Value) -> Vec<WorkspaceInfo> {
    let Some(workspaces) = snapshot.get("workspaces").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out: Vec<WorkspaceInfo> = workspaces
        .iter()
        .filter_map(|w| {
            Some(WorkspaceInfo {
                workspace_id: str_of(w, "workspace_id")?,
                label: str_of(w, "label").unwrap_or_default(),
                number: w.get("number").and_then(Value::as_u64).unwrap_or(0),
                pane_count: w.get("pane_count").and_then(Value::as_u64).unwrap_or(0),
            })
        })
        .collect();
    out.sort_by_key(|w| w.number);
    out
}

fn dirs_or_home() -> PathBuf {
    std::env::var("HOME").map(PathBuf::from).unwrap_or_default()
}
