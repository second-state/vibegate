//! `vibegate herdr-tui` — fullscreen gateway dashboard for the herdr plugin
//! pane: ratatui-kit runtime (state/events) + plain ratatui rendering.

use crate::raw_http;
use ratatui_kit::prelude::*;
use ratatui_kit::{
    crossterm::event::{Event, KeyCode, KeyEventKind},
    ratatui::{
        layout::{Constraint, Direction, Flex, Layout, Rect},
        style::{Style, Stylize},
        text::Line,
        widgets::{Block, Paragraph, Widget},
    },
};

fn log_path() -> String {
    std::env::var("VIBEGATE_LOG_FILE").unwrap_or_else(|_| {
        format!(
            "{}/.vibegate/logs/vibegate_rCURRENT.log",
            std::env::var("HOME").unwrap_or_default()
        )
    })
}

fn start_daemon(addr: String, log: String) {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::thread::spawn(move || {
            use std::os::unix::process::CommandExt;
            let _ = std::process::Command::new(exe)
                .args(["run", "-d", "--bind", &addr])
                .env("VIBEGATE_LOG_FILE", log)
                .stdin(std::process::Stdio::null())
                .process_group(0)
                .status();
        });
    }
}

fn stop_gateway_async(bind: String) {
    tokio::spawn(async move {
        let _ = raw_http(&bind, "POST /api/shutdown HTTP/1.1\r\nContent-Length: 0\r\n").await;
    });
}

/// Three box-styled "buttons"; the selected one is highlighted.
struct ButtonsWidget {
    selected: usize,
}

impl Widget for ButtonsWidget {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let labels = ["s · Start", "x · Stop", "q · Quit"];
        let chunks: [Rect; 3] = Layout::horizontal([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .flex(Flex::SpaceBetween)
        .areas(area);
        for (i, chunk) in chunks.iter().enumerate() {
            let style = if i == self.selected {
                Style::new().cyan().bold()
            } else {
                Style::new().dark_gray()
            };
            let block = Block::bordered().border_style(style);
            let inner = block.inner(*chunk);
            block.render(*chunk, buf);
            // label as content, centered — not as the block title
            Paragraph::new(Line::from(labels[i]).style(style).centered()).render(inner, buf);
        }
    }
}

/// Entry: run the dashboard fullscreen until the user quits.
pub async fn run() -> anyhow::Result<()> {
    ratatui_kit::element!(HerdrTui)
        .fullscreen()
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Header box + log box + buttons row.
#[derive(Clone)]
struct DashboardWidget {
    status: String,
    addr: String,
    log: String,
    selected: usize,
    tail: Vec<Line<'static>>,
}

impl Widget for DashboardWidget {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let chunks: [Rect; 3] = Layout::vertical([
            Constraint::Length(6),
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .areas(area);

        let status_style = if self.status.starts_with("RUNNING") {
            Style::new().green()
        } else {
            Style::new().red()
        };
        let hblock = Block::bordered()
            .title(Line::from(" vibegate herdr-tui ").cyan().bold())
            .border_style(Style::new().cyan());
        let hinner = hblock.inner(chunks[0]);
        hblock.render(chunks[0], buf);
        Paragraph::new(vec![
            Line::styled(format!(" gateway : {}", self.status), status_style),
            Line::from(format!(" bind    : {}", self.addr)),
            Line::from(format!(" log     : {}", self.log)),
        ])
        .render(hinner, buf);

        let lblock = Block::bordered().title(Line::from(" log tail ").cyan());
        let linner = lblock.inner(chunks[1]);
        lblock.render(chunks[1], buf);
        Paragraph::new(self.tail).render(linner, buf);

        ButtonsWidget {
            selected: self.selected,
        }
        .render(chunks[2], buf);
    }
}

#[component]
fn HerdrTui(mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let bind = std::env::var("VIBEGATE_BIND").unwrap_or_else(|_| "http://127.0.0.1:8480".into());
    let addr = bind
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .to_string();
    let log = log_path();

    let mut status = hooks.use_state(|| "checking…".to_string());
    let mut selected = hooks.use_state(|| 0_usize);

    // Probe the gateway once a second; writes wake the render loop.
    let probe_bind = bind.clone();
    hooks.use_future(async move {
        loop {
            let line = match raw_http(&probe_bind, "GET /api/status HTTP/1.1\r\n").await {
                Ok(buf) => {
                    let body = buf.split("\r\n\r\n").nth(1).unwrap_or("");
                    let json = body
                        .trim_start_matches(|c: char| c != '{')
                        .trim_end_matches(|c: char| c != '}');
                    serde_json::from_str::<serde_json::Value>(json)
                        .map(|v| {
                            format!(
                                "RUNNING  {} v{}",
                                v["name"].as_str().unwrap_or("vibegate"),
                                v["version"].as_str().unwrap_or("?")
                            )
                        })
                        .unwrap_or_else(|_| "NOT RUNNING".into())
                }
                Err(_) => "NOT RUNNING".into(),
            };
            status.set(line);
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });

    let mut exit = hooks.use_exit();
    // ←/→ + Enter drive the buttons; s / x / q stay as direct shortcuts.
    let ev_addr = addr.clone();
    let ev_bind = bind.clone();
    let ev_log = log.clone();
    hooks.use_event_handler(EventScope::Current, EventPriority::Normal, move |event| {
        let Event::Key(key) = event else {
            return EventResult::Ignored;
        };
        if key.kind != KeyEventKind::Press {
            return EventResult::Ignored;
        }
        let idx = selected.get();
        match key.code {
            KeyCode::Left => {
                selected.set((idx + 2) % 3);
                EventResult::Consumed
            }
            KeyCode::Right => {
                selected.set((idx + 1) % 3);
                EventResult::Consumed
            }
            KeyCode::Enter => match idx {
                0 => {
                    start_daemon(ev_addr.clone(), ev_log.clone());
                    EventResult::Consumed
                }
                1 => {
                    stop_gateway_async(ev_bind.clone());
                    EventResult::Consumed
                }
                _ => {
                    exit();
                    EventResult::Consumed
                }
            },
            KeyCode::Char('s') => {
                start_daemon(ev_addr.clone(), ev_log.clone());
                EventResult::Consumed
            }
            KeyCode::Char('x') => {
                stop_gateway_async(ev_bind.clone());
                EventResult::Consumed
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                exit();
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    });

    // Fresh tail + status on every render (renders fire ~1/s via the probe).
    let status_value = status.read().clone();
    let tail: Vec<Line> = std::fs::read_to_string(&log)
        .map(|c| {
            let lines: Vec<&str> = c.lines().collect();
            let start = lines.len().saturating_sub(12);
            lines[start..]
                .iter()
                .map(|l| Line::from(l.to_string()))
                .collect()
        })
        .unwrap_or_else(|_| vec![Line::from("(no log yet)".to_string())]);

    element!(
        View(flex_direction: Direction::Vertical, height: Constraint::Percentage(100)) {
            widget(DashboardWidget {
                status: status_value,
                addr: addr.clone(),
                log: log.clone(),
                selected: selected.get(),
                tail,
            })
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    #[test]
    fn button_label_is_centered_in_content() {
        let area = Rect::new(0, 0, 90, 3);
        let mut buf = Buffer::empty(area);
        ButtonsWidget { selected: 0 }.render(area, &mut buf);

        // content row (y=1): label centered inside the first button's inner area
        let row: String = (0..90).map(|x| buf[(x, 1)].symbol().to_string()).collect();
        println!("content row: {row:?}");
        assert!(row.contains("s · Start"), "label missing: {row:?}");
        let start = row.find('s').unwrap();
        // first chunk ≈ 29 wide, inner 27; (27 - 9) / 2 = 9 → label starts ≈ col 10
        assert!(start >= 8, "label not centered: starts at col {start}");
        // corners intact on the top border
        let top: String = (0..90).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(top.starts_with('┌') && top.ends_with('┐'), "corners eaten: {top:?}");
    }
}
