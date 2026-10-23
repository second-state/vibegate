mod files;
mod herdr;
mod pty;
mod term;
mod ws;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context as _, Result};
use axum::extract::ConnectInfo;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use clap::{Parser, Subcommand};
use serde_json::json;

use herdr::Herdr;

/// Web frontend assets. Release builds embed them into the binary; debug
/// builds live-read `web/dist` from disk (rebuild the frontend, refresh).
#[derive(rust_embed::RustEmbed)]
#[folder = "web/dist"]
struct WebAssets;

fn mime_of(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "wasm" => "application/wasm",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match WebAssets::get(path) {
        Some(file) => {
            let mime = mime_of(path);
            (
                [(header::CONTENT_TYPE, mime)],
                axum::body::Body::from(file.data),
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

/// GET /api/status — liveness probe: package name + version.
async fn status_handler() -> Response {
    (
        StatusCode::OK,
        json!({
            "name": env!("CARGO_PKG_NAME"),
            "version": env!("CARGO_PKG_VERSION"),
        })
        .to_string(),
    )
        .into_response()
}

/// POST /api/shutdown — graceful exit, loopback callers only (the gateway
/// normally binds 0.0.0.0, so an unfiltered shutdown would be a LAN kill
/// switch).
async fn shutdown_handler(ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>) -> Response {
    if !addr.ip().is_loopback() {
        return (
            StatusCode::FORBIDDEN,
            json!({"error": "shutdown only allowed from localhost"}).to_string(),
        )
            .into_response();
    }
    // Reply first, then exit — gives the response time to flush.
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        tracing::info!("shutdown requested via /api/shutdown, exiting");
        std::process::exit(0);
    });
    (StatusCode::OK, json!({"type": "shutdown"}).to_string()).into_response()
}

/// Web gateway for herdr terminal sessions.
#[derive(Parser, Debug)]
#[command(name = "vibegate", version, about)]
struct Cli {
    /// herdr socket path (default ~/.config/herdr/herdr.sock)
    #[arg(long, global = true, env = "VIBEGATE_HERDR_SOCKET")]
    herdr_socket: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Print the panes herdr currently exposes (debug helper).
    List,
    /// Run the gateway: WebSocket streaming + file API + static web.
    #[command(after_help = "EXAMPLES:
  vibegate run                                          # 127.0.0.1:8480, embedded web, file root = $HOME
  vibegate run --bind 0.0.0.0:8480                      # LAN — phone opens http://<this-ip>:8480
  vibegate run --file-root ~/workspace --file-root /tmp # restrict the file API to given roots
  vibegate --herdr-socket /tmp/herdr.sock run           # custom herdr socket")]
    Run {
        /// Bind address for HTTP/WS.
        #[arg(long, env = "VIBEGATE_BIND", default_value = "127.0.0.1:8480")]
        bind: String,
        /// Directory of the built web frontend to serve at `/` (debug builds
        /// only — release builds embed the assets).
        #[arg(long)]
        #[cfg_attr(not(debug_assertions), arg(hide = true))]
        web_dir: Option<PathBuf>,
        /// Allowed filesystem root for the file API (repeatable).
        #[arg(long = "file-root")]
        file_roots: Vec<PathBuf>,
    },
    /// Stop a running gateway (loopback only — see /api/shutdown).
    Stop {
        /// Base URL of the running gateway.
        #[arg(long, default_value = "http://127.0.0.1:8480")]
        bind: String,
    },
    /// Show whether a gateway is running and which version.
    Status {
        /// Base URL of the running gateway.
        #[arg(long, default_value = "http://127.0.0.1:8480")]
        bind: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    let herdr = Arc::new(Herdr::new(
        cli.herdr_socket.unwrap_or_else(Herdr::default_socket),
    ));

    match cli.command {
        Commands::List => {
            let panes = herdr.panes().await?;
            println!(
                "{:<10} {:<28} {:<8} {:<30} cwd",
                "workspace", "terminal_id", "agent", "title"
            );
            for p in panes {
                println!(
                    "{:<10} {:<28} {:<8} {:<30} {}",
                    p.workspace_id,
                    p.terminal_id,
                    p.agent.as_deref().unwrap_or("-"),
                    truncate(&p.title, 28),
                    p.cwd,
                );
            }
            Ok(())
        }
        Commands::Run { bind, web_dir, file_roots } => run(herdr, bind, web_dir, file_roots).await,
        Commands::Stop { bind } => stop_gateway(&bind).await,
        Commands::Status { bind } => status_gateway(&bind).await,
    }
}

/// Raw minimal HTTP — one control call doesn't justify a client crate.
async fn raw_http(base: &str, request: &str) -> Result<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let host_port = base
        .strip_prefix("http://")
        .ok_or_else(|| anyhow::anyhow!("only http:// base URLs are supported"))?
        .trim_end_matches('/')
        .to_string();
    let mut stream = tokio::net::TcpStream::connect(&host_port)
        .await
        .with_context(|| format!("connecting to {host_port} — is the gateway running?"))?;
    let req = format!("{request}\r\nHost: {host_port}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).await?;
    let mut buf = String::new();
    stream.read_to_string(&mut buf).await?;
    Ok(buf)
}

async fn stop_gateway(base: &str) -> Result<()> {
    let buf = raw_http(base, "POST /api/shutdown HTTP/1.1\r\nContent-Length: 0\r\n").await?;
    if buf.starts_with("HTTP/1.1 200") || buf.starts_with("HTTP/1.0 200") {
        println!("vibegate stopped");
    } else if buf.contains(" 403") {
        anyhow::bail!("shutdown rejected — only allowed from localhost");
    } else {
        anyhow::bail!("unexpected response: {}", buf.lines().next().unwrap_or(""));
    }
    Ok(())
}

async fn status_gateway(base: &str) -> Result<()> {
    let buf = raw_http(base, "GET /api/status HTTP/1.1\r\n").await?;
    // The body may be chunk-encoded; grab from the first '{' to the last '}'.
    let body = buf.split("\r\n\r\n").nth(1).unwrap_or("");
    let json = body
        .trim_start_matches(|c: char| c != '{')
        .trim_end_matches(|c: char| c != '}');
    let v: serde_json::Value = serde_json::from_str(json)
        .with_context(|| format!("unexpected response: {}", buf.lines().next().unwrap_or("")))?;
    let name = v["name"].as_str().unwrap_or("?");
    let version = v["version"].as_str().unwrap_or("?");
    let mine = env!("CARGO_PKG_VERSION");
    if version == mine {
        println!("{name} {version} — running");
    } else {
        println!("{name} — running {version}, this binary is {mine}");
        println!("consider `vibegate stop` + start to switch versions");
    }
    Ok(())
}

async fn run(
    herdr: Arc<Herdr>,
    bind: String,
    web_dir: Option<PathBuf>,
    file_roots: Vec<PathBuf>,
) -> Result<()> {
    // Session poller: rebuild the sessions JSON every 2 s; bump a watch
    // counter on change (events.subscribe would be nicer — later).
    let sessions_rw = Arc::new(tokio::sync::RwLock::new(String::new()));
    let (changed_tx, changed_rx) = tokio::sync::watch::channel(0u64);
    {
        let herdr = herdr.clone();
        let sessions_rw = sessions_rw.clone();
        tokio::spawn(async move {
            let mut last = String::new();
            let mut counter: u64 = 0;
            loop {
                let json = match herdr.snapshot().await {
                    Ok(snapshot) => {
                        let panes: Vec<_> = herdr::panes_from_snapshot(&snapshot)
                            .into_iter()
                            .map(|p| {
                                json!({
                                    "sid": p.terminal_id,
                                    "pane_id": p.pane_id,
                                    "title": p.title,
                                    "agent": p.agent,
                                    "agent_status": p.agent_status,
                                    "cwd": p.cwd,
                                    "workspace_id": p.workspace_id,
                                })
                            })
                            .collect();
                        let workspaces: Vec<_> = herdr::workspaces_from_snapshot(&snapshot)
                            .into_iter()
                            .map(|w| {
                                json!({
                                    "id": w.workspace_id,
                                    "label": w.label,
                                    "number": w.number,
                                    "pane_count": w.pane_count,
                                })
                            })
                            .collect();
                        json!({"type":"sessions","panes":panes,"workspaces":workspaces}).to_string()
                    }
                    Err(e) => {
                        tracing::warn!("snapshot failed: {e}");
                        json!({"type":"sessions","panes":[],"workspaces":[],"error":e.to_string()})
                            .to_string()
                    }
                };
                if json != last {
                    *sessions_rw.write().await = json.clone();
                    counter += 1;
                    let _ = changed_tx.send(counter);
                    last = json;
                }
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        });
    }

    let roots = if file_roots.is_empty() {
        vec![std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/"))]
    } else {
        file_roots
    };
    let files_state = Arc::new(files::FilesState { roots });
    let ws_state = Arc::new(ws::WsState {
        herdr: herdr.clone(),
        sessions: sessions_rw,
        changed: changed_rx,
    });

    let ws_routes = axum::Router::new()
        .route("/ws", axum::routing::get(ws::ws_handler))
        .route("/term/{terminal_id}", axum::routing::get(term::term_handler))
        .with_state(ws_state);

    let file_routes = axum::Router::new()
        .route("/api/files", axum::routing::get(files::list_handler))
        .route(
            "/api/file",
            axum::routing::get(files::download_handler).put(files::upload_handler),
        )
        .route("/api/upload/status", axum::routing::get(files::upload_status_handler))
        .route("/api/status", axum::routing::get(status_handler))
        .route("/api/shutdown", axum::routing::post(shutdown_handler))
        .with_state(files_state)
        // Uploads stream to disk unbounded (the default 2 MB extractor cap
        // would 413 phone photos).
        .layer(axum::extract::DefaultBodyLimit::disable());

    let mut app = ws_routes.merge(file_routes);

    {
        #[cfg(debug_assertions)]
        {
            // Debug: --web-dir serves the directory live (unchanged behavior);
            // without it the embedded assets live-read from web/dist.
            if let Some(dir) = web_dir.filter(|d| d.exists()) {
                tracing::info!("serving web frontend from {}", dir.display());
                app = app.fallback_service(tower_http::services::ServeDir::new(dir));
            } else {
                tracing::info!("serving embedded web frontend (debug: live-reads web/dist)");
                app = app.fallback(static_handler);
            }
        }
        #[cfg(not(debug_assertions))]
        {
            // Release: assets are baked into the binary — single-file deploy.
            if web_dir.is_some() {
                tracing::info!("--web-dir ignored: release builds embed the web frontend");
            }
            tracing::info!("serving embedded web frontend");
            app = app.fallback(static_handler);
        }
    }

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("vibegate listening on http://{bind}");
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}
