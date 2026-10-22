//! `GET /ws` — control channel.
//!
//! Text-JSON only. Server pushes the pane list on connect and whenever a
//! 2-second snapshot poll detects a change; the client can create and kill
//! workspaces. Terminal bytes do not flow here — each open window gets its
//! own raw WebSocket at `/term/<terminal_id>`.

use std::sync::Arc;

use anyhow::Result;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;

use crate::herdr::Herdr;

#[derive(Clone)]
pub struct WsState {
    pub herdr: Arc<Herdr>,
    /// Latest `{"type":"sessions",...}` JSON, rebuilt on every change.
    pub sessions: Arc<tokio::sync::RwLock<String>>,
    pub changed: tokio::sync::watch::Receiver<u64>,
}

pub async fn ws_handler(
    State(state): State<Arc<WsState>>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    upgrade.on_upgrade(move |sock| control_sock(state, sock))
}

async fn control_sock(state: Arc<WsState>, mut sock: WebSocket) {
    let _ = sock
        .send(Message::text(json!({"type":"hello","protocol":1}).to_string()))
        .await;
    let _ = sock.send(Message::text(sessions_json(&state).await)).await;

    let (mut tx, mut rx) = sock.split();
    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<Message>(64);

    // Writer pump: everything the handlers produce goes through out_tx.
    let writer = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    let mut changed = state.changed.clone();
    let mut last_seen = *changed.borrow_and_update();

    loop {
        let mut wait_changed = changed.clone();
        let seen = last_seen;
        tokio::select! {
            msg = rx.next() => {
                let Some(Ok(Message::Text(text))) = msg else { break };
                if handle_control(&state, &out_tx, text.as_str()).await.is_err() {
                    break;
                }
            }
            fired = async move {
                // Consume the watch Ref inside this block: its guard is !Send
                // and must not be held across the handler's awaits below.
                wait_changed.wait_for(move |v| *v != seen).await.is_ok()
            } => {
                if !fired {
                    break;
                }
                last_seen = *changed.borrow_and_update();
                let json = sessions_json(&state).await;
                if out_tx.send(Message::text(json)).await.is_err() {
                    break;
                }
            }
        }
    }

    drop(out_tx);
    let _ = writer.await;
}

async fn sessions_json(state: &WsState) -> String {
    state.sessions.read().await.clone()
}

#[derive(Deserialize)]
struct ControlMsg {
    #[serde(rename = "type")]
    kind: String,
    label: Option<String>,
    cwd: Option<String>,
    workspace_id: Option<String>,
}

async fn handle_control(
    state: &Arc<WsState>,
    out: &tokio::sync::mpsc::Sender<Message>,
    text: &str,
) -> Result<()> {
    let Ok(msg) = serde_json::from_str::<ControlMsg>(text) else {
        return send_err(out, "unrecognized control message").await;
    };
    match msg.kind.as_str() {
        "create" => {
            let label = msg.label.unwrap_or_else(|| "web".to_string());
            let result = state
                .herdr
                .create_workspace(&label, msg.cwd.as_deref())
                .await?;
            let workspace_id = result
                .get("workspace")
                .and_then(|w| w.get("workspace_id"))
                .and_then(serde_json::Value::as_str)
                .or_else(|| result.get("workspace_id").and_then(serde_json::Value::as_str))
                .map(String::from);
            // Find the fresh pane: poll the snapshot for a pane inside the new
            // workspace (create replies don't carry the pane/terminal ids).
            let mut sid = None;
            for _ in 0..10 {
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                for p in state.herdr.panes().await? {
                    if Some(&p.workspace_id) == workspace_id.as_ref() {
                        sid = Some(p.terminal_id);
                        break;
                    }
                }
                if sid.is_some() {
                    break;
                }
            }
            let Some(sid) = sid else {
                return send_err(out, "workspace created but no pane appeared").await;
            };
            let reply = json!({
                "type": "created",
                "sid": sid,
                "workspace_id": workspace_id,
            });
            out.send(Message::text(reply.to_string())).await?;
            Ok(())
        }
        "kill" => {
            let Some(ws_id) = msg.workspace_id.clone() else {
                return send_err(out, "kill requires workspace_id").await;
            };
            state.herdr.close_workspace(&ws_id).await?;
            out.send(Message::text(json!({"type":"killed"}).to_string())).await?;
            Ok(())
        }
        "ping" => {
            out.send(Message::text(json!({"type":"pong"}).to_string())).await?;
            Ok(())
        }
        other => {
            send_err(out, &format!("unknown control type {other:?}")).await
        }
    }
}

async fn send_err(out: &tokio::sync::mpsc::Sender<Message>, message: &str) -> Result<()> {
    let reply = json!({"type":"error","message":message});
    Ok(out.send(Message::text(reply.to_string())).await?)
}
