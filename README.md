# vibefleet

A single-binary web gateway that exposes [herdr](https://github.com/herdrdev/herdr)-managed
terminal sessions through an infinite canvas UI — a mobile-friendly alternative to
sshx. Terminals, images and markdown (with mermaid diagrams) live as draggable
windows you can pan, zoom and rearrange from a phone or desktop.

The release binary embeds the entire web frontend: copy it anywhere, run it, done.

> Chinese guide: [docs/README.zh-CN.md](docs/README.zh-CN.md).

## Requirements

- A running [herdr](https://github.com/herdrdev/herdr) server (socket defaults to
  `~/.config/herdr/herdr.sock`; `HERDR_SOCKET_PATH` / `HERDR_CONFIG_ROOT` are respected)
- A browser (desktop or mobile)

## Quick start

```bash
./build-release.sh            # bundles web/dist into the binary
./target/release/vibefleet run # foreground; Ctrl-C to stop
```

Open `http://localhost:8480`. To serve your LAN (e.g. a phone):

```bash
vibefleet run --bind 0.0.0.0:8480
```

### Install / update

```bash
curl -fsSL https://raw.githubusercontent.com/second-state/vibefleet/main/install.sh | bash
vibefleet update               # self-update from GitHub Releases
```

Or clone and run the script locally:

```bash
./install.sh                  # latest release into ~/.cargo/bin
```

Adjust the release repository (`REPO` in `install.sh` / `src/update.rs`) if you
host vibefleet under a different name.

## Command line

```
vibefleet run [-d] [--bind ADDR] [--file-root DIR]... [--web-dir DIR]
vibefleet stop   [--bind URL]
vibefleet status [--bind URL]
vibefleet update [TAG]
vibefleet list
```

| Command | What it does |
|---|---|
| `run` | Start the gateway in the foreground |
| `run -d` | Start it as a detached daemon and return immediately |
| `stop` | Gracefully stop a running gateway (loopback callers only) |
| `status` | Show whether a gateway is running and which version |
| `tui` | Fullscreen dashboard: gateway status, log tail, start/stop/quit |
| `update [TAG]` | Self-update from GitHub Releases (default: latest stable) |
| `list` | Print the herdr panes the gateway can see (debug helper) |

Useful flags and defaults:

- `--bind` — listen address, default `127.0.0.1:8480`
- `--file-root DIR` — directories the file API may touch; repeatable, defaults to `$HOME`
- `--herdr-socket PATH` — herdr socket override

Environment variables: `VIBEFLEET_BIND`, `VIBEFLEET_HERDR_SOCKET`,
`VIBEFLEET_LOG_DIR` (daemon log directory), `RUST_LOG` (log filter, default `info`).

## Daemon

```bash
vibefleet run -d --bind 0.0.0.0:8480   # background + return to your shell
vibefleet status                       # vibefleet 0.1.0 — running
vibefleet stop                         # graceful shutdown
```

The daemon appends logs to `~/.vibefleet/logs/` — current file
`vibefleet_rCURRENT.log`, rotated at 10 MB with 5-day retention (set
`VIBEFLEET_LOG_DIR` to relocate the directory). Starting a second instance is refused with
the running version. Foreground `run` stays available for systemd-style
supervisors.

## herdr plugin

vibefleet ships a plugin manifest (`herdr-plugin.toml`). Link a checkout into
herdr:

```bash
herdr plugin link /path/to/vibefleet
```

That adds one action to herdr's menus — **Open vibefleet** — which opens a
full-coverage pane running `try-run.sh`: on first use it installs vibefleet
(arrow-key menu: local checkout or GitHub release), then shows the `tui`
dashboard (gateway status, log tail, start/stop/quit). Closing the pane stops
the TUI.

## Web UI

- **Connect** — the page uses the same origin it was served from; use the
  hamburger menu → gateway URL to point it elsewhere
- **Sessions** (top right) — herdr workspaces as a tree; `+` creates terminals,
  rows open on the canvas, `×` closes a pane; agent status is shown verbatim
  (`idle / working / blocked / done / unknown`)
- **Canvas** — drag to pan, pinch (two fingers) or `ctrl`-scroll to zoom,
  double-click a window to center it
- **Window headers** — drag to move, corner to resize, `×` closes the window
  (the remote session keeps running), `⟳` reconnects a dropped stream
- **Keybar** — floating toolbar under the focused terminal: sticky
  `Ctrl / Alt / Shift` (the next key, from the keybar or the soft keyboard,
  carries it), arrows, scroll keys, `1×` zoom reset. Reposition or hide it in
  the menu (Float / Top / Hidden)
- **Canvas mode** (![](docs/canvas-mode-icon.svg) next to Files) — all
  windows become click-through except their title bars: drags and pinches
  navigate the canvas instead of typing, while close / reconnect / dragging
  a window by its title bar keeps working. Toggle with the button or a
  two-finger tap anywhere; the keybar hides while it is on
- **Files** — browse the configured roots; `⇧ Upload` sends files (any size,
  resumable — failed uploads get a `↻ Retry` button); `⇩` downloads; click an
  image or a `.md` file to open it as a canvas window (markdown renders
  tables, code and mermaid diagrams)
- **Menu** — terminal themes (Tokyo Night, Dracula, Nord, Gruvbox, One Dark,
  Sakura, Sequoia Moonlight), canvas/terminal background images with opacity,
  keybar placement, 中文 / English

## HTTP API

| Path | Method | Purpose |
|---|---|---|
| `/ws` | WS | control channel: pane/workspace lists, create/close |
| `/term/{terminal_id}` | WS | raw terminal stream (`?cols=&rows=`) |
| `/api/files?path=` | GET | directory listing (restricted to file roots) |
| `/api/file?path=` | GET | download (streamed) |
| `/api/file?path=` | PUT | upload — unlimited size, resumable via `X-Upload-Token` / `X-Upload-Total` |
| `/api/upload/status?path=&token=` | GET | bytes already received for a resumable upload |
| `/api/status` | GET | `{"name":"vibefleet","version":"…"}` liveness probe |
| `/api/shutdown` | POST | graceful exit — **loopback callers only** |

Use `vibefleet stop` instead of calling `/api/shutdown` by hand.
