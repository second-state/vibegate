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
use axum::http::StatusCode;
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
    /// `1` → no Content-Disposition: attachment (used by inline previews).
    inline: Option<String>,
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
    (code, json!({"error": message}).to_string()).into_response()
}

/// Sidecar file remembering the upload token of an in-progress (resumable)
/// upload: `.<filename>.vbp`. Filtered out of directory listings.
fn sidecar_path(p: &Path) -> PathBuf {
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    p.with_file_name(format!(".{name}.vbp"))
}

/// Content type by extension — `<img>` tags only render SVG (and other
/// non-raster formats) when served with a proper image/* type.
fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("bmp") => "image/bmp",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("txt") | Some("md") | Some("log") => "text/plain; charset=utf-8",
        Some("json") => "application/json",
        Some("pdf") => "application/pdf",
        Some("mp4") | Some("m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mov") => "video/quicktime",
        Some("mkv") => "video/x-matroska",
        Some("ogv") => "video/ogg",
        Some("avi") => "video/x-msvideo",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("ogg") | Some("oga") => "audio/ogg",
        Some("flac") => "audio/flac",
        Some("m4a") | Some("aac") => "audio/mp4",
        _ => "application/octet-stream",
    }
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
        // Hide resumable-upload sidecar files.
        if entry.file_name().to_string_lossy().ends_with(".vbp") {
            continue;
        }
        let Ok(meta) = entry.metadata().await else {
            continue;
        };
        entries.push(json!({
            "name": entry.file_name().to_string_lossy(),
            "is_dir": meta.is_dir(),
            "size": meta.len(),
            "mtime": meta.modified().ok().and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()),
        }));
    }
    entries.sort_by(|a, b| {
        let dir = b
            .get("is_dir")
            .and_then(serde_json::Value::as_bool)
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

/// GET /api/file?path=... — with `Range` support (206 partial content) so
/// video/audio previews can seek, and `&inline=1` to skip the
/// Content-Disposition: attachment that makes plain downloads save to disk.
pub async fn download_handler(
    State(state): State<Arc<FilesState>>,
    Query(q): Query<FilePath>,
    req_headers: axum::http::HeaderMap,
) -> Response {
    use axum::http::header;
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    let path = match resolve(&state, &q.path) {
        Ok(p) => p,
        Err(r) => return r,
    };
    if !path.is_file() {
        return err(StatusCode::BAD_REQUEST, "not a file");
    }
    let mut file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };
    let total = file.metadata().await.map(|m| m.len()).unwrap_or(0);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string());

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static(content_type(&path)),
    );
    if q.inline.as_deref() != Some("1") {
        if let Ok(v) = header::HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")) {
            headers.insert(header::CONTENT_DISPOSITION, v);
        }
    }
    headers.insert(
        header::ACCEPT_RANGES,
        header::HeaderValue::from_static("bytes"),
    );

    // Parse `bytes=start-end` / `bytes=start-` / `bytes=-suffix` (first
    // range only; multipart ranges fall back to a full 200).
    let range = req_headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("bytes="))
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string());

    if let Some(spec) = range {
        let (start, end) = match parse_range(&spec, total) {
            Some(r) => r,
            None => {
                // Unsatisfiable (or malformed): advertise the real size.
                let mut resp = err(StatusCode::RANGE_NOT_SATISFIABLE, "bad range");
                if let Ok(v) = header::HeaderValue::from_str(&format!("bytes */{total}")) {
                    resp.headers_mut().insert(header::CONTENT_RANGE, v);
                }
                return resp;
            }
        };
        let len = end - start + 1;
        if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
            return err(StatusCode::INTERNAL_SERVER_ERROR, "seek failed");
        }
        let stream = tokio_util::io::ReaderStream::new(file.take(len));
        let mut resp = (
            StatusCode::PARTIAL_CONTENT,
            [(
                header::CONTENT_RANGE,
                format!("bytes {start}-{end}/{total}"),
            )],
            Body::from_stream(stream),
        )
            .into_response();
        if let Ok(v) = header::HeaderValue::from_str(&len.to_string()) {
            resp.headers_mut().insert(header::CONTENT_LENGTH, v);
        }
        resp.headers_mut().extend(headers);
        return resp;
    }

    let stream = tokio_util::io::ReaderStream::new(file);
    (headers, Body::from_stream(stream)).into_response()
}

/// `bytes=start-end` / `bytes=start-` / `bytes=-suffix` → inclusive
/// (start, end) clamped to the file size; None when malformed or empty.
fn parse_range(spec: &str, total: u64) -> Option<(u64, u64)> {
    if total == 0 {
        return None;
    }
    let (first, last) = spec.split_once('-')?;
    let (start, end) = if first.is_empty() {
        // suffix: last N bytes
        let n: u64 = last.parse().ok()?;
        let n = n.min(total);
        (total - n, total - 1)
    } else {
        let start: u64 = first.parse().ok()?;
        let end = if last.is_empty() {
            total - 1
        } else {
            last.parse::<u64>().ok()?.min(total - 1)
        };
        (start, end)
    };
    if start > end || start >= total {
        return None;
    }
    Some((start, end))
}

/// GET /api/upload/status?path=...&token=... — how many bytes of a resumable
/// upload does the server already hold? A token mismatch (stale partial from
/// a different session) discards the partial and reports 0.
#[derive(Deserialize)]
pub struct UploadStatusQuery {
    path: String,
    token: String,
}

pub async fn upload_status_handler(
    State(state): State<Arc<FilesState>>,
    Query(q): Query<UploadStatusQuery>,
) -> Response {
    let path = match resolve(&state, &q.path) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let sidecar = sidecar_path(&path);
    let meta = match tokio::fs::metadata(&path).await {
        Ok(m) if m.is_file() => m,
        _ => {
            let _ = tokio::fs::remove_file(&sidecar).await;
            return (StatusCode::OK, json!({"size": 0}).to_string()).into_response();
        }
    };
    let saved = tokio::fs::read_to_string(&sidecar)
        .await
        .unwrap_or_default();
    if saved.trim() == q.token {
        (StatusCode::OK, json!({"size": meta.len()}).to_string()).into_response()
    } else {
        let _ = tokio::fs::remove_file(&path).await;
        let _ = tokio::fs::remove_file(&sidecar).await;
        (StatusCode::OK, json!({"size": 0}).to_string()).into_response()
    }
}

/// PUT /api/file?path=... — streams the body to disk (no size limit; nothing
/// is buffered whole, so multi-hundred-MB uploads cost no RAM).
///
/// Resumable protocol: with `X-Upload-Token` (and `X-Upload-Total`), bytes are
/// APPENDED to any partial left by an interrupted attempt carrying the same
/// token; the partial and its sidecar survive a broken connection and are only
/// cleaned up once the announced total has been reached. Without the header
/// the write truncates and an aborted transfer removes the partial (legacy).
pub async fn upload_handler(
    State(state): State<Arc<FilesState>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<FilePath>,
    body: axum::body::Body,
) -> Response {
    let path = match resolve(&state, &q.path) {
        Ok(p) => p,
        Err(r) => return r,
    };
    if path.is_dir() {
        return err(StatusCode::BAD_REQUEST, "path is a directory");
    }
    let token = headers
        .get("x-upload-token")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let total: Option<u64> = headers
        .get("x-upload-total")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok());

    use tokio::io::AsyncWriteExt;
    let mut file = if let Some(tok) = &token {
        let sidecar = sidecar_path(&path);
        let saved = tokio::fs::read_to_string(&sidecar)
            .await
            .unwrap_or_default();
        let existing = tokio::fs::metadata(&path).await.ok().map(|m| m.len());
        if saved.trim() == tok && existing.is_some() {
            // Defensive: the body we're about to append must land exactly at
            // the announced total. (A client that lost its offset could send
            // the whole file again — appending that would corrupt the file.)
            if let (Some(have), Some(cl)) = (
                existing,
                headers
                    .get("content-length")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok()),
            ) {
                if let Some(t) = total {
                    if have + cl != t {
                        return err(
                            StatusCode::CONFLICT,
                            "offset mismatch — query /api/upload/status and retry",
                        );
                    }
                }
            }
            // Same interrupted upload: append to what we already have.
            match tokio::fs::OpenOptions::new().append(true).open(&path).await {
                Ok(f) => f,
                Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
            }
        } else {
            // Fresh start: truncate and mark the partial as ours.
            if tokio::fs::write(&sidecar, tok).await.is_err() {
                return err(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "cannot mark partial upload",
                );
            }
            match tokio::fs::File::create(&path).await {
                Ok(f) => f,
                Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
            }
        }
    } else {
        match tokio::fs::File::create(&path).await {
            Ok(f) => f,
            Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        }
    };

    let mut size: u64 = 0;
    let mut stream = body.into_data_stream();
    while let Some(chunk) = futures_util::StreamExt::next(&mut stream).await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                if token.is_none() {
                    // Legacy upload: don't leave a partial behind.
                    let _ = tokio::fs::remove_file(&path).await;
                }
                // Token mode keeps partial + sidecar so the next attempt resumes.
                return err(StatusCode::BAD_REQUEST, &format!("upload aborted: {e}"));
            }
        };
        if let Err(e) = file.write_all(&chunk).await {
            return err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string());
        }
        size += chunk.len() as u64;
    }
    let _ = file.flush().await;

    if token.is_some() {
        // Request finished cleanly: once the announced total is on disk the
        // upload is complete — drop the sidecar.
        let len = tokio::fs::metadata(&path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        if total.map_or(true, |t| len >= t) {
            let _ = tokio::fs::remove_file(sidecar_path(&path)).await;
        }
    }

    let resp = json!({"type":"uploaded","path":path.display().to_string(),"size":size});
    (StatusCode::OK, resp.to_string()).into_response()
}
