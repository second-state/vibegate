//! Attach-PTY wrapper (adapted from vibetty's terminal/pty.rs, Unix-only).
//!
//! Spawns `herdr terminal attach <tid>` (or any command) inside a PTY and
//! exposes its byte stream: an async `read()` that returns chunks as they
//! arrive, a buffered writer, resize, and kill. This is the same shape as
//! vibetty's TerminalEvent::PtyOutput path.

use std::io::{Read, Write};

use anyhow::Result;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use tokio::sync::{mpsc, oneshot};

pub(crate) type WriteMsg = (Vec<u8>, oneshot::Sender<std::io::Result<()>>);

pub struct GatePty {
    master: Box<dyn portable_pty::MasterPty + Send>,
    child: Option<Box<dyn portable_pty::Child + Send + Sync>>,
    read_rx: mpsc::Receiver<Vec<u8>>,
    write_tx: mpsc::Sender<WriteMsg>,
}

/// Write/resize/kill half of a split [`GatePty`].
pub struct PtyWriteHalf {
    // Mutex so `&PtyWriteHalf` is Send (MasterPty is Send but not Sync); the
    // lock is only ever taken for the brief resize call.
    master: std::sync::Mutex<Box<dyn portable_pty::MasterPty + Send>>,
    child: Option<Box<dyn portable_pty::Child + Send + Sync>>,
    write_tx: mpsc::Sender<WriteMsg>,
}

impl PtyWriteHalf {
    pub async fn write_all(&self, buf: &[u8]) -> std::io::Result<()> {
        let (ack_tx, ack_rx) = oneshot::channel();
        self.write_tx
            .send((buf.to_vec(), ack_tx))
            .await
            .map_err(|_| std::io::Error::other("pty writer gone"))?;
        ack_rx.await.map_err(|_| std::io::Error::other("pty writer dropped ack"))?
    }

    pub fn resize(&self, rows: u16, cols: u16) -> std::io::Result<()> {
        self.master
            .lock()
            .unwrap()
            .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(std::io::Error::other)
    }
}

impl Drop for PtyWriteHalf {
    fn drop(&mut self) {
        // Closing our master end makes the attach client's PTY reads fail, so
        // it exits; kill explicitly to not wait on that.
        if let Some(child) = self.child.as_ref() {
            let _ = child.clone_killer().kill();
        }
    }
}

impl GatePty {
    /// Consume the PTY into an owned read half and a write half, so the two
    /// pump loops can each own their side.
    pub fn split(self) -> (mpsc::Receiver<Vec<u8>>, PtyWriteHalf) {
        (
            self.read_rx,
            PtyWriteHalf {
                master: std::sync::Mutex::new(self.master),
                child: self.child,
                write_tx: self.write_tx,
            },
        )
    }


    pub async fn spawn(
        program: &str,
        args: &[&str],
        size: (u16, u16),
    ) -> Result<Self> {
        let (rows, cols) = size;
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(program);
        for arg in args {
            cmd.arg(arg);
        }
        for (key, value) in std::env::vars_os() {
            cmd.env(key, value);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("FORCE_COLOR", "1");
        if let Ok(cwd) = std::env::current_dir() {
            cmd.cwd(cwd);
        }

        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);

        let reader = pair.master.try_clone_reader()?;
        let (read_tx, read_rx) = mpsc::channel::<Vec<u8>>(256);
        tokio::task::spawn_blocking(move || {
            let mut buf = [0u8; 16384];
            let mut r = reader;
            loop {
                match r.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if read_tx.blocking_send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let writer = pair.master.take_writer()?;
        let (write_tx, mut write_rx) = mpsc::channel::<WriteMsg>(128);
        tokio::task::spawn_blocking(move || {
            let mut w = writer;
            while let Some((buf, ack)) = write_rx.blocking_recv() {
                let res = w.write_all(&buf).and_then(|_| w.flush());
                let _ = ack.send(res);
            }
        });

        Ok(Self {
            master: pair.master,
            child: Some(child),
            read_rx,
            write_tx,
        })
    }

}
