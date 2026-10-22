//! `GET /term/<terminal_id>` — raw terminal WebSocket.
//!
//! One connection per open terminal window. On upgrade we spawn
//! `herdr terminal attach <tid>` inside a PTY (sized from the query params)
//! and pump bytes both ways: server→client is the pane's rendered output,
//! client→server is keyboard input. herdr repaints the full screen on attach
//! and on resize, so there is no baseline protocol and nothing to parse —
//! the stream passes through verbatim, exactly like vibetty's PtyOutput.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;

use crate::pty::GatePty;

#[derive(Deserialize)]
pub struct TermQuery {
    cols: Option<u16>,
    rows: Option<u16>,
}

pub async fn term_handler(
    Path(terminal_id): Path<String>,
    Query(q): Query<TermQuery>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    let cols = q.cols.unwrap_or(80).clamp(8, 500);
    let rows = q.rows.unwrap_or(24).clamp(2, 200);
    upgrade.on_upgrade(move |sock| terminal_sock(terminal_id, cols, rows, sock))
}

async fn terminal_sock(terminal_id: String, cols: u16, rows: u16, sock: WebSocket) {
    let pty = match GatePty::spawn(
        "herdr",
        &["terminal", "attach", &terminal_id],
        (rows, cols),
    )
    .await
    {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("[{terminal_id}] attach failed: {e}");
            let mut sock = sock;
            let _ = sock
                .send(Message::text(
                    r#"{"type":"error","message":"attach failed"}"#.to_string(),
                ))
                .await;
            return;
        }
    };
    tracing::info!("[{terminal_id}] attached ({cols}x{rows})");
    let (mut read_rx, write_half) = pty.split();

    // Pump both directions until either side ends.
    let (mut tx, mut rx) = sock.split();
    let read_loop = async move {
        loop {
            let chunk = read_rx.recv().await.unwrap_or_default();
            if chunk.is_empty() {
                // Attach ended: pane closed, or herdr dropped the client.
                let _ = tx.send(Message::Binary(axum::body::Bytes::new())).await;
                let _ = tx.close().await;
                break;
            }
            if tx.send(Message::Binary(chunk.into())).await.is_err() {
                break; // viewer gone
            }
        }
    };

    let write_loop = async move {
        while let Some(msg) = rx.next().await {
            match msg {
                Ok(Message::Binary(b)) => {
                    if write_half.write_all(&b).await.is_err() {
                        break;
                    }
                }
                Ok(Message::Text(t)) => {
                    // Text on the data socket = {"cols":C,"rows":R} resize.
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
                        let cols = v.get("cols").and_then(serde_json::Value::as_u64);
                        let rows = v.get("rows").and_then(serde_json::Value::as_u64);
                        if let (Some(c), Some(r)) = (cols, rows) {
                            let _ = write_half.resize(r as u16, c as u16);
                        }
                    }
                }
                Ok(Message::Close(_)) | Err(_) => break,
                Ok(_) => {} // ignore ping/pong frames on the data socket
            }
        }
    };

    tokio::select! {
        _ = read_loop => {},
        _ = write_loop => {},
    }
    tracing::info!("[{terminal_id}] detached");
}
