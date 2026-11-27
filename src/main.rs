mod files;
mod hc;
mod herdr;
mod pty;
mod term;
mod tui;
mod update;
mod ws;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context as _, Result};
use axum::extract::ConnectInfo;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use clap::{Parser, Subcommand, ValueEnum};
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

/// Session backend: classic herdr socket, or herdr-core (`hc`).
#[derive(ValueEnum, Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Backend {
    #[default]
    Herdr,
    Hc,
}

/// Web gateway for herdr terminal sessions.
#[derive(Parser, Debug)]
#[command(name = "vibefleet", version, about)]
struct Cli {
    /// herdr socket path (default ~/.config/herdr/herdr.sock)
    #[arg(long, global = true, env = "VIBEFLEET_HERDR_SOCKET")]
    herdr_socket: Option<PathBuf>,

    /// run: session backend (herdr socket or herdr-core `hc`)
    #[arg(
        long,
        global = true,
        value_enum,
        env = "VIBEFLEET_BACKEND",
        default_value = "herdr"
    )]
    backend: Backend,

    /// run: bind address to listen on; stop/status: gateway to talk to
    /// (0.0.0.0 dials loopback).
    #[arg(
        long,
        global = true,
        env = "VIBEFLEET_BIND",
        default_value = "127.0.0.1:8480"
    )]
    bind: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Print the panes herdr currently exposes (debug helper).
    List,
    /// Run the gateway: WebSocket streaming + file API + static web.
    #[command(after_help = "EXAMPLES:
  vibefleet run                                          # 127.0.0.1:8480, embedded web, file root = $HOME
  vibefleet run --bind 0.0.0.0:8480                      # LAN — phone opens http://<this-ip>:8480
  vibefleet run --file-root ~/workspace --file-root /tmp # restrict the file API to given roots
  vibefleet --herdr-socket /tmp/herdr.sock run           # custom herdr socket")]
    Run {
        /// Spawn a detached daemon and return immediately.
        #[arg(short = 'd', long = "detach")]
        detach: bool,
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
    Stop,
    /// Fullscreen status dashboard for the herdr plugin pane.
    Tui,
    /// Show whether a gateway is running and which version.
    Status,
    /// Self-update from GitHub Releases (installs like install.sh).
    Update {
        /// Release tag to install (default: latest stable).
        #[arg(default_value = "latest")]
        tag: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let _logger_guard = init_logger();

    let cli = Cli::parse();
    let herdr_socket = cli.herdr_socket.clone();
    let herdr = Arc::new(Herdr::new(
        herdr_socket.clone().unwrap_or_else(Herdr::default_socket),
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
        Commands::Run {
            detach,
            web_dir,
            file_roots,
        } => {
            if detach {
                spawn_detached(
                    &cli.bind,
                    cli.backend,
                    web_dir.as_ref(),
                    &file_roots,
                    herdr_socket.as_deref(),
                )
                .await
            } else {
                run(herdr, cli.backend, cli.bind.clone(), web_dir, file_roots).await
            }
        }
        Commands::Stop => stop_gateway(&loopback_base(&cli.bind)).await,
        Commands::Status => status_gateway(&loopback_base(&cli.bind)).await,
        Commands::Tui => tui::run(cli.bind.clone(), cli.backend, cli.herdr_socket.clone()).await,
        Commands::Update { tag } => update::run(Some(tag)).await.map_err(anyhow::Error::msg),
    }
}

/// Foreground runs log to stdio; the detached daemon (VIBEFLEET_LOG_DIR set
/// by the launcher) logs to `<dir>/vibefleet_rCURRENT.log` with vibetty-style
/// rotation: append, 10MB per file, keep 5 days (the `_rCURRENT` suffix is
/// flexi_logger's own naming for the current file). RUST_LOG filters both.
fn init_logger() -> Option<flexi_logger::LoggerHandle> {
    match std::env::var("VIBEFLEET_LOG_DIR") {
        Ok(dir) if !dir.is_empty() => {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).ok();
            use flexi_logger::{Cleanup, Criterion, FileSpec, Logger, Naming, WriteMode};
            Some(
                Logger::try_with_env_or_str("info")
                    .expect("parse RUST_LOG specification")
                    .log_to_file(
                        FileSpec::default()
                            .suppress_timestamp()
                            .directory(dir)
                            .basename("vibefleet"),
                    )
                    .format_for_files(flexi_logger::detailed_format) // 带时间戳
                    .append()
                    .write_mode(WriteMode::BufferAndFlush)
                    .rotate(
                        Criterion::Size(10_000_000), // 10MB per file
                        Naming::Numbers,
                        Cleanup::KeepForDays(5),
                    )
                    .start()
                    .expect("start file logger"),
            )
        }
        _ => {
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "info".into()),
                )
                .init();
            None
        }
    }
}

/// Launcher for `run -d`: refuse double-starts (probe /api/status first),
/// then exec ourselves detached. The child runs the same foreground `run`;
/// only VIBEFLEET_LOG_DIR tells it to log to file.
async fn spawn_detached(
    bind: &str,
    backend: Backend,
    web_dir: Option<&PathBuf>,
    file_roots: &[PathBuf],
    herdr_socket: Option<&std::path::Path>,
) -> Result<()> {
    let base = loopback_base(bind);
    if let Ok(buf) = raw_http(&base, "GET /api/status HTTP/1.1\r\n").await {
        let body = buf.split("\r\n\r\n").nth(1).unwrap_or("");
        let json = body
            .trim_start_matches(|c: char| c != '{')
            .trim_end_matches(|c: char| c != '}');
        let version = serde_json::from_str::<serde_json::Value>(json)
            .ok()
            .and_then(|v| v["version"].as_str().map(String::from))
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
        println!("vibefleet {version} is already running on {bind}");
        return Ok(());
    }

    let exe = std::env::current_exe().context("locate current executable")?;
    // An explicit VIBEFLEET_LOG_DIR from the caller wins; default otherwise.
    // All tracing goes through flexi_logger into the dir — stdio is nulled,
    // no stray redirect file next to the real log.
    let log_dir = match std::env::var("VIBEFLEET_LOG_DIR") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => std::env::var("HOME")
            .map(|h| PathBuf::from(h).join(".vibefleet").join("logs"))
            .unwrap_or_else(|_| PathBuf::from(".")),
    };
    std::fs::create_dir_all(&log_dir).ok();

    use std::os::unix::process::CommandExt;
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("run")
        .arg("--bind")
        .arg(bind)
        .env("VIBEFLEET_LOG_DIR", &log_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0); // detach from the terminal's process group
    for r in file_roots {
        cmd.arg("--file-root").arg(r);
    }
    if let Some(wd) = web_dir {
        cmd.arg("--web-dir").arg(wd);
    }
    if let Some(sock) = herdr_socket {
        cmd.arg("--herdr-socket").arg(sock);
    }
    if backend == Backend::Hc {
        cmd.arg("--backend").arg("hc");
    }
    cmd.spawn().context("spawn detached daemon")?;
    println!(
        "vibefleet started in background on {bind} (log dir: {})",
        log_dir.display()
    );
    Ok(())
}

/// Client-side base URL for a bind address: 0.0.0.0 means "listen on all
/// interfaces", so dial loopback instead of 0.0.0.0.
fn loopback_base(bind: &str) -> String {
    let bare = bind
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let (host, port) = bare.rsplit_once(':').unwrap_or((bare, "80"));
    let host = if host.is_empty() || host == "0.0.0.0" {
        "127.0.0.1"
    } else {
        host
    };
    format!("http://{host}:{port}")
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
        println!("vibefleet stopped");
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
        println!("consider `vibefleet stop` + start to switch versions");
    }
    Ok(())
}

/// Build the `{"type":"sessions",...}` payload from the hc terminal map —
/// same shape the herdr poller emits so the frontend stays unchanged.
/// Workspace groups are derived client-side from the `workspace` label.
fn hc_sessions_json(terms: &std::collections::BTreeMap<String, hc::HcTerminal>) -> String {
    let mut ws_counts: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
    let mut panes = Vec::with_capacity(terms.len());
    for t in terms.values() {
        if !t.workspace.is_empty() {
            *ws_counts.entry(t.workspace.as_str()).or_default() += 1;
        }
        panes.push(json!({
            "sid": t.address,
            "pane_id": t.address,
            "title": t.title,
            "agent": t.agent,
            "agent_status": t.agent_status.clone().unwrap_or_else(|| "unknown".into()),
            "cwd": t.cwd,
            "workspace_id": t.workspace,
        }));
    }
    let workspaces: Vec<_> = ws_counts
        .iter()
        .enumerate()
        .map(|(i, (label, count))| {
            json!({
                // id must equal the panes' workspace_id (the label value) —
                // the frontend groups panes by matching it against the
                // workspace id.
                "id": label,
                "label": label,
                "number": i + 1,
                "pane_count": count,
            })
        })
        .collect();
    json!({"type":"sessions","panes":panes,"workspaces":workspaces}).to_string()
}

async fn run(
    herdr: Arc<Herdr>,
    backend: Backend,
    bind: String,
    web_dir: Option<PathBuf>,
    file_roots: Vec<PathBuf>,
) -> Result<()> {
    let sessions_rw = Arc::new(tokio::sync::RwLock::new(String::new()));
    let (changed_tx, changed_rx) = tokio::sync::watch::channel(0u64);
    match backend {
        Backend::Herdr => {
            // Session poller: rebuild the sessions JSON every 2 s; bump a
            // watch counter on change (events.subscribe would be nicer — later).
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
                            json!({"type":"sessions","panes":panes,"workspaces":workspaces})
                                .to_string()
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
        Backend::Hc => {
            // herdr-core backend: one long-running `hc events` child feeds
            // parsed events; the consumer keeps the terminal map and
            // rebuilds the same sessions payload (labels.workspace stands
            // in for herdr's workspaces).
            let sessions_rw = sessions_rw.clone();
            tokio::spawn(async move {
                let (tx, mut rx) = tokio::sync::mpsc::channel::<hc::HcEvent>(256);
                tokio::spawn(hc::run_feed(tx));
                tracing::info!("hc backend: session feed from `hc events`");
                let mut terms: std::collections::BTreeMap<String, hc::HcTerminal> =
                    std::collections::BTreeMap::new();
                let mut last = String::new();
                let mut counter: u64 = 0;
                loop {
                    let Some(ev) = rx.recv().await else { return };
                    match ev {
                        hc::HcEvent::Snapshot(list) => {
                            terms.clear();
                            for t in list {
                                terms.insert(t.address.clone(), t);
                            }
                        }
                        hc::HcEvent::Upsert(t) => {
                            terms.insert(t.address.clone(), t);
                        }
                        hc::HcEvent::Remove(address) => {
                            terms.remove(&address);
                        }
                        hc::HcEvent::Status {
                            terminal,
                            app,
                            state,
                        } => {
                            if let Some(t) = terms.get_mut(&terminal) {
                                t.agent = app;
                                t.agent_status = state;
                            }
                        }
                        hc::HcEvent::Title { terminal, title } => {
                            if let Some(t) = terms.get_mut(&terminal) {
                                t.title = title;
                            }
                        }
                        hc::HcEvent::Labels { terminal, labels } => {
                            if let Some(t) = terms.get_mut(&terminal) {
                                t.labels = labels.clone();
                                t.workspace = labels.get("workspace").cloned().unwrap_or_default();
                            }
                        }
                        hc::HcEvent::Foreground { terminal, cwd } => {
                            if let (Some(t), Some(c)) = (terms.get_mut(&terminal), cwd) {
                                t.cwd = c;
                            }
                        }
                        hc::HcEvent::Ended { terminal } => {
                            // herdr parity: finished/lost panes leave the
                            // session list (kept ones return via `restarted`
                            // → Upsert when their shell comes back).
                            terms.remove(&terminal);
                        }
                    }
                    let json = hc_sessions_json(&terms);
                    if json != last {
                        *sessions_rw.write().await = json.clone();
                        counter += 1;
                        let _ = changed_tx.send(counter);
                        last = json;
                    }
                }
            });
        }
    }

    let roots = if file_roots.is_empty() {
        vec![
            std::env::var("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("/")),
        ]
    } else {
        file_roots
    };
    let files_state = Arc::new(files::FilesState { roots });
    let ws_state = Arc::new(ws::WsState {
        herdr: herdr.clone(),
        backend,
        sessions: sessions_rw,
        changed: changed_rx,
    });

    let ws_routes = axum::Router::new()
        .route("/ws", axum::routing::get(ws::ws_handler))
        .route(
            "/term/{terminal_id}",
            axum::routing::get(term::term_handler),
        )
        .with_state(ws_state);

    let file_routes = axum::Router::new()
        .route("/api/files", axum::routing::get(files::list_handler))
        .route(
            "/api/file",
            axum::routing::get(files::download_handler).put(files::upload_handler),
        )
        .route(
            "/api/upload/status",
            axum::routing::get(files::upload_status_handler),
        )
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
    tracing::info!("vibefleet listening on http://{bind}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
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
