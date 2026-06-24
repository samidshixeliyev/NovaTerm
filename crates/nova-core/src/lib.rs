//! `nova-core` — session orchestration / PTY pump.
//!
//! Owns the lifecycle of every terminal session: it spawns the ConPTY, runs a
//! blocking *reader* thread and a *pump* thread that coalesces output and
//! broadcasts it as base64 [`CoreEvent::Output`]. Rendering/VT parsing is done
//! by the frontend's terminal engine (xterm.js); the core stays a fast, dumb
//! byte pipe. Input bytes from the UI ([`Core::write_text`]) are written back to
//! the PTY, and resizes are forwarded to the pseudoconsole.
//!
//! ([`keymap`] and [`Core::input`] remain for non-xterm consumers that want the
//! core to encode keys itself.)

#![forbid(unsafe_code)]

pub mod keymap;

use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use base64::Engine;
use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::Mutex;

use nova_config::Config;
use nova_protocol::{CoreEvent, InputEvent, ResizeEvent, SessionId, SpawnParams};
use nova_pty::{CommandBuilder, Pty, PtySize};
use thiserror::Error;
use tokio::sync::broadcast;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("no profile matched and no default is configured")]
    NoProfile,
    #[error("session {0} not found")]
    NoSession(SessionId),
    #[error(transparent)]
    Pty(#[from] nova_pty::PtyError),
}

pub type Result<T> = std::result::Result<T, CoreError>;

/// Control messages sent to a session's pump thread.
enum Msg {
    Bytes(Vec<u8>),
    Eof,
    Shutdown,
}

struct Session {
    pty: Arc<Mutex<Pty>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    to_proc: Sender<Msg>,
    app_cursor: Arc<AtomicBool>,
}

struct CoreInner {
    sessions: Mutex<HashMap<SessionId, Session>>,
    events: broadcast::Sender<CoreEvent>,
    config: Mutex<Config>,
}

/// The engine. Clone-cheap (`Arc` inside); share one across the app.
#[derive(Clone)]
pub struct Core {
    inner: Arc<CoreInner>,
}

impl Core {
    /// Create a core with the given configuration. Returns the core and an
    /// event receiver the UI layer subscribes to.
    #[must_use]
    pub fn new(config: Config) -> (Core, broadcast::Receiver<CoreEvent>) {
        let (tx, rx) = broadcast::channel(4096);
        let inner = Arc::new(CoreInner {
            sessions: Mutex::new(HashMap::new()),
            events: tx,
            config: Mutex::new(config),
        });
        (Core { inner }, rx)
    }

    /// Subscribe to the event stream (one receiver per consumer).
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<CoreEvent> {
        self.inner.events.subscribe()
    }

    /// Replace the active configuration (e.g. after a reload).
    pub fn set_config(&self, config: Config) {
        *self.inner.config.lock() = config;
    }

    /// Spawn a new session per `params`, returning its id.
    pub fn spawn(&self, params: SpawnParams) -> Result<SessionId> {
        let cmd = {
            let cfg = self.inner.config.lock();
            let profile = cfg
                .profiles
                .resolve(params.profile_id.as_deref())
                .ok_or(CoreError::NoProfile)?;
            let mut cmd = CommandBuilder::new(&profile.shell).args(profile.args.clone());
            if let Some(cwd) = &params.cwd {
                cmd = cmd.cwd(cwd);
            }
            cmd
        };

        let size = PtySize {
            cols: params.cols.max(1),
            rows: params.rows.max(1),
        };
        let mut pty = Pty::spawn(&cmd, size)?;
        let reader = pty.take_reader().expect("fresh pty has a reader");
        let writer = pty.take_writer().expect("fresh pty has a writer");

        let sid = SessionId::new();
        let pid = pty.try_wait().ok().flatten();
        let pty = Arc::new(Mutex::new(pty));
        let app_cursor = Arc::new(AtomicBool::new(false));

        let (to_proc, from_ctrl) = bounded::<Msg>(1024);

        // Reader thread: blocking reads -> bytes messages; EOF -> Msg::Eof.
        {
            let to_proc = to_proc.clone();
            let mut reader = reader;
            thread::Builder::new()
                .name(format!("pty-read-{sid}"))
                .spawn(move || {
                    use std::io::Read;
                    let mut buf = [0u8; 16 * 1024];
                    loop {
                        match reader.read(&mut buf) {
                            Ok(0) => {
                                let _ = to_proc.send(Msg::Eof);
                                break;
                            }
                            Ok(n) => {
                                if to_proc.send(Msg::Bytes(buf[..n].to_vec())).is_err() {
                                    break;
                                }
                            }
                            Err(_) => {
                                let _ = to_proc.send(Msg::Eof);
                                break;
                            }
                        }
                    }
                })
                .expect("spawn reader thread");
        }

        // Pump thread: coalesces PTY output at a cadence and broadcasts it raw.
        {
            let events = self.inner.events.clone();
            let pty = Arc::clone(&pty);
            let tick = Duration::from_millis(
                self.inner.config.lock().rendering.frame_tick_ms.max(1) as u64,
            );
            thread::Builder::new()
                .name(format!("pty-pump-{sid}"))
                .spawn(move || run_pump(sid, tick, from_ctrl, events, pty))
                .expect("spawn pump thread");
        }

        let session = Session {
            pty,
            writer: Arc::new(Mutex::new(Box::new(writer))),
            to_proc,
            app_cursor,
        };
        self.inner.sessions.lock().insert(sid, session);

        let _ = self.inner.events.send(CoreEvent::Spawned {
            session: sid,
            pid: pid.map_or(0, |c| c as u32),
        });
        Ok(sid)
    }

    /// Deliver a UI input event to a session.
    pub fn input(&self, session: SessionId, event: InputEvent) -> Result<()> {
        let sessions = self.inner.sessions.lock();
        let s = sessions
            .get(&session)
            .ok_or(CoreError::NoSession(session))?;
        match event {
            InputEvent::Key(ev) => {
                let bytes = keymap::encode_key(&ev, s.app_cursor.load(Ordering::Relaxed));
                if !bytes.is_empty() {
                    let mut w = s.writer.lock();
                    let _ = w.write_all(&bytes);
                    let _ = w.flush();
                }
            }
            InputEvent::Paste { text } => {
                let mut w = s.writer.lock();
                let _ = w.write_all(text.as_bytes());
                let _ = w.flush();
            }
            InputEvent::Resize(size) => {
                drop(sessions);
                self.resize(session, size)?;
            }
            // Mouse reporting and viewport scrolling are handled UI-side in the MVP.
            InputEvent::Mouse(_) | InputEvent::ScrollViewport { .. } => {}
        }
        Ok(())
    }

    /// Write raw bytes to a session's PTY (used by tests and shell integration).
    pub fn write(&self, session: SessionId, bytes: &[u8]) -> Result<()> {
        let sessions = self.inner.sessions.lock();
        let s = sessions
            .get(&session)
            .ok_or(CoreError::NoSession(session))?;
        let mut w = s.writer.lock();
        let _ = w.write_all(bytes);
        let _ = w.flush();
        Ok(())
    }

    /// Write UI text/keystroke bytes to a session's PTY. xterm.js already encodes
    /// keys into the correct escape sequences, so the frontend sends that string
    /// here verbatim.
    pub fn write_text(&self, session: SessionId, data: &str) -> Result<()> {
        self.write(session, data.as_bytes())
    }

    /// Resize a session (both the ConPTY and the grid model).
    pub fn resize(&self, session: SessionId, size: ResizeEvent) -> Result<()> {
        let sessions = self.inner.sessions.lock();
        let s = sessions
            .get(&session)
            .ok_or(CoreError::NoSession(session))?;
        let cols = size.cols.max(1);
        let rows = size.rows.max(1);
        s.pty.lock().resize(PtySize { cols, rows })?;
        Ok(())
    }

    /// Kept for API compatibility. With an external VT engine the frontend owns
    /// the screen buffer, so there is no full frame to re-request — this is a
    /// no-op beyond validating the session exists.
    pub fn request_full_frame(&self, session: SessionId) -> Result<()> {
        let sessions = self.inner.sessions.lock();
        sessions.get(&session).ok_or(CoreError::NoSession(session))?;
        Ok(())
    }

    /// Close a session, terminating its child.
    pub fn close(&self, session: SessionId) -> Result<()> {
        let s = self
            .inner
            .sessions
            .lock()
            .remove(&session)
            .ok_or(CoreError::NoSession(session))?;
        let _ = s.to_proc.send(Msg::Shutdown);
        let _ = s.pty.lock().kill();
        Ok(())
    }

    /// Number of live sessions.
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.inner.sessions.lock().len()
    }
}

/// Coalesce PTY output at `tick` cadence and broadcast it as base64 `Output`
/// events. Bursts collapse into a single event so a flood (`cat huge.log`)
/// doesn't spam the IPC channel.
fn run_pump(
    sid: SessionId,
    tick: Duration,
    rx: Receiver<Msg>,
    events: broadcast::Sender<CoreEvent>,
    pty: Arc<Mutex<Pty>>,
) {
    let b64 = base64::engine::general_purpose::STANDARD;
    loop {
        let mut buf: Vec<u8> = Vec::new();
        let mut eof = false;

        match rx.recv_timeout(tick) {
            Ok(Msg::Bytes(b)) => buf.extend_from_slice(&b),
            Ok(Msg::Eof) | Ok(Msg::Shutdown) => eof = true,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        }
        while let Ok(m) = rx.try_recv() {
            match m {
                Msg::Bytes(b) => buf.extend_from_slice(&b),
                Msg::Eof | Msg::Shutdown => eof = true,
            }
        }

        if !buf.is_empty() {
            let _ = events.send(CoreEvent::Output { session: sid, base64: b64.encode(&buf) });
        }
        if eof {
            break;
        }
    }

    let code = pty.lock().try_wait().ok().flatten().unwrap_or(0);
    let _ = events.send(CoreEvent::Exited { session: sid, code });
}
