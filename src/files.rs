//! `GET/PUT /api/files` — file browsing / download / upload over plain HTTP,
//! restricted to configured roots (default: $HOME).
//!
//! Routes (all take `?path=<absolute-or-root-relative>`):
//! - `GET  /api/files`        → JSON listing of a directory
//! - `GET  /api/file`         → download a file (streamed)
//! - `PUT  /api/file`         → upload/overwrite a file (request body = bytes)

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

#[derive(Clone)]
pub struct FilesState {
    pub roots: Vec<PathBuf>,
}

#[derive(Deserialize)]
pub struct FilePath {
    path: String,
}

fn resolve(state: &FilesState, raw: &str) -> Result<PathBuf, Response> {
    let p = Path::new(raw);
    let candidate = if p.is_absolute() {
        p.to_path_buf()
    } else {
        // Relative paths resolve against the first root.
        state.roots[0].join(p)
    };

    // Lexically normalize `.` / `..` first.
    let mut norm = PathBuf::new();
    for comp in candidate.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                norm.pop();
            }
            c => norm.push(c.as_os_str()),
        }
    }

    // Canonicalize the longest existing prefix (uploads target files that do
    // not exist yet), then re-append the remainder.
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    let mut anc = norm.clone();
    let canon = loop {
        match anc.canonicalize() {
            Ok(c) => break c,
            Err(_) => {
                let last = anc.file_name().map(|f| f.to_os_string());
                if !anc.pop() || last.is_none() {
                    return Err(err(StatusCode::NOT_FOUND, "path not found"));
                }
                tail.push(last.expect("checked above"));
            }
        }
    };
    let mut full = canon;
    for seg in tail.into_iter().rev() {
        full.push(seg);
    }

    if !state.roots.iter().any(|root| full.starts_with(root)) {
        return Err(err(StatusCode::FORBIDDEN, "path outside allowed roots"));
    }
    Ok(full)
}

fn err(code: StatusCode, message: &str) -> Response {
    (code, message.to_string()).into_response()
}

/// GET /api/files?path=...
pub async fn list_handler(
    State(state): State<Arc<FilesState>>,
    Query(q): Query<FilePath>,
) -> Response {
    let path = match resolve(&state, &q.path) {
        Ok(p) => p,
        Err(r) => return r,
    };
    if path.is_file() {
        // Convenience: listing a file returns its own stat.
        let meta = match tokio::fs::metadata(&path).await {
            Ok(m) => m,
            Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        };
        let body = json!({
            "type": "file",
            "path": path.display().to_string(),
            "size": meta.len(),
        });
        return (StatusCode::OK, body.to_string()).into_response();
    }
    let mut rd = match tokio::fs::read_dir(&path).await {
        Ok(rd) => rd,
        Err(e) => return err(StatusCode::BAD_REQUEST, &e.to_string()),
    };
    let mut entries = Vec::new();
    while let Ok(Some(entry)) = rd.next_entry().await {
        let Ok(meta) = entry.metadata().await else { continue };
        entries.push(json!({
            "name": entry.file_name().to_string_lossy(),
            "is_dir": meta.is_dir(),
            "size": meta.len(),
            "mtime": meta.modified().ok().and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()),
        }));
    }
    entries.sort_by(|a, b| {
        let dir = b.get("is_dir").and_then(serde_json::Value::as_bool)
            .cmp(&a.get("is_dir").and_then(serde_json::Value::as_bool));
        dir.then_with(|| {
            a.get("name")
                .and_then(serde_json::Value::as_str)
                .cmp(&b.get("name").and_then(serde_json::Value::as_str))
        })
    });
    let body = json!({
        "type": "dir",
        "path": path.display().to_string(),
        "entries": entries,
    });
    (StatusCode::OK, body.to_string()).into_response()
}

/// GET /api/file?path=...
pub async fn download_handler(
    State(state): State<Arc<FilesState>>,
    Query(q): Query<FilePath>,
) -> Response {
    let path = match resolve(&state, &q.path) {
        Ok(p) => p,
        Err(r) => return r,
    };
    if !path.is_file() {
        return err(StatusCode::BAD_REQUEST, "not a file");
    }
    let file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string());
    let stream = tokio_util::io::ReaderStream::new(file);
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("application/octet-stream"));
    if let Ok(v) = header::HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")) {
        headers.insert(header::CONTENT_DISPOSITION, v);
    }
    (headers, Body::from_stream(stream)).into_response()
}

/// PUT /api/file?path=...
pub async fn upload_handler(
    State(state): State<Arc<FilesState>>,
    Query(q): Query<FilePath>,
    body: axum::body::Bytes,
) -> Response {
    let path = match resolve(&state, &q.path) {
        Ok(p) => p,
        Err(r) => return r,
    };
    if path.is_dir() {
        return err(StatusCode::BAD_REQUEST, "path is a directory");
    }
    match tokio::fs::write(&path, &body).await {
        Ok(()) => {
            let resp = json!({"type":"uploaded","path":path.display().to_string(),"size":body.len()});
            (StatusCode::OK, resp.to_string()).into_response()
        }
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}
