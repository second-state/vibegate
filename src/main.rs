mod files;
mod herdr;
mod pty;
mod term;
mod ws;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::json;

use herdr::Herdr;

/// Web gateway for herdr terminal sessions.
#[derive(Parser, Debug)]
#[command(name = "vibegate", version, about)]
struct Cli {
    /// herdr socket path (default ~/.config/herdr/herdr.sock)
    #[arg(long, global = true)]
    herdr_socket: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Print the panes herdr currently exposes (debug helper).
    List,
    /// Run the gateway: WebSocket streaming + file API + static web.
    Run {
        /// Bind address for HTTP/WS.
        #[arg(long, default_value = "127.0.0.1:8480")]
        bind: String,
        /// Directory of the built web frontend to serve at `/`.
        #[arg(long)]
        web_dir: Option<PathBuf>,
        /// Allowed filesystem root for the file API (repeatable).
        #[arg(long = "file-root")]
        file_roots: Vec<PathBuf>,
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
    }
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
                let json = match herdr.panes().await {
                    Ok(panes) => {
                        let panes: Vec<_> = panes
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
                        json!({"type":"sessions","panes":panes}).to_string()
                    }
                    Err(e) => {
                        tracing::warn!("snapshot failed: {e}");
                        json!({"type":"sessions","panes":[],"error":e.to_string()}).to_string()
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
        .with_state(files_state);

    let mut app = ws_routes.merge(file_routes);

    if let Some(dir) = web_dir.filter(|d| d.exists()) {
        tracing::info!("serving web frontend from {}", dir.display());
        app = app.fallback_service(tower_http::services::ServeDir::new(dir));
    }

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("vibegate listening on http://{bind}");
    axum::serve(listener, app).await?;
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
