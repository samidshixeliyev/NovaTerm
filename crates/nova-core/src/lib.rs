//! `nova-core` — session orchestration.
//!
//! Owns the lifecycle of every terminal session: it spawns the ConPTY, runs a
//! blocking *reader* thread and a frame-cadenced *processor* thread, feeds bytes
//! through the [`nova_terminal::Terminal`] model, and broadcasts
//! [`CoreEvent`]s (frames, title/cwd, bell, exit) that the UI layer forwards to
//! the webview. Input from the UI is encoded by [`keymap`] and written back to
//! the PTY.

#![forbid(unsafe_code)]

pub mod keymap;

use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::Mutex;

use nova_config::Config;
use nova_protocol::{CoreEvent, InputEvent, ResizeEvent, SessionId, SpawnParams};
use nova_pty::{CommandBuilder, Pty, PtySize};
use nova_terminal::{TermEvent, Terminal};
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

/// Control messages sent to a session's processor thread.
enum Msg {
    Bytes(Vec<u8>),
    Resize { cols: u16, rows: u16 },
    RequestFull,
    Eof,
    Shutdown,
}

struct Session {
    pty: Arc<Mutex<Pty>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    to_proc: Sender<Msg>,
    app_cursor: Arc<AtomicBool>,
    cols: u16,
    rows: u16,
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

        // Processor thread: owns the terminal model, coalesces at frame cadence.
        {
            let events = self.inner.events.clone();
            let pty = Arc::clone(&pty);
            let app_cursor = Arc::clone(&app_cursor);
            let tick = Duration::from_millis(
                self.inner.config.lock().rendering.frame_tick_ms.max(1) as u64,
            );
            let cols = size.cols;
            let rows = size.rows;
            let scrollback = self.inner.config.lock().terminal.scrollback_lines as usize;
            thread::Builder::new()
                .name(format!("pty-proc-{sid}"))
                .spawn(move || {
                    run_processor(
                        sid, cols, rows, scrollback, tick, from_ctrl, events, pty, app_cursor,
                    )
                })
                .expect("spawn processor thread");
        }

        let session = Session {
            pty,
            writer: Arc::new(Mutex::new(Box::new(writer))),
            to_proc,
            app_cursor,
            cols: size.cols,
            rows: size.rows,
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

    /// Resize a session (both the ConPTY and the grid model).
    pub fn resize(&self, session: SessionId, size: ResizeEvent) -> Result<()> {
        let mut sessions = self.inner.sessions.lock();
        let s = sessions
            .get_mut(&session)
            .ok_or(CoreError::NoSession(session))?;
        let cols = size.cols.max(1);
        let rows = size.rows.max(1);
        s.pty.lock().resize(PtySize { cols, rows })?;
        s.cols = cols;
        s.rows = rows;
        let _ = s.to_proc.send(Msg::Resize { cols, rows });
        Ok(())
    }

    /// Ask a session to re-emit a full frame (e.g. after the UI reattaches).
    pub fn request_full_frame(&self, session: SessionId) -> Result<()> {
        let sessions = self.inner.sessions.lock();
        let s = sessions
            .get(&session)
            .ok_or(CoreError::NoSession(session))?;
        let _ = s.to_proc.send(Msg::RequestFull);
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

#[allow(clippy::too_many_arguments)]
fn run_processor(
    sid: SessionId,
    cols: u16,
    rows: u16,
    scrollback: usize,
    tick: Duration,
    rx: Receiver<Msg>,
    events: broadcast::Sender<CoreEvent>,
    pty: Arc<Mutex<Pty>>,
    app_cursor: Arc<AtomicBool>,
) {
    let mut term = Terminal::new(cols, rows, scrollback);
    let mut force_full = true; // first frame paints the whole screen
    let mut eof = false;

    loop {
        // Block up to one tick for the first message, then drain the backlog so
        // a burst of output collapses into a single frame.
        let mut batch: Vec<Msg> = Vec::new();
        match rx.recv_timeout(tick) {
            Ok(m) => batch.push(m),
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        }
        while let Ok(m) = rx.try_recv() {
            batch.push(m);
        }

        for m in batch {
            match m {
                Msg::Bytes(b) => term.feed(&b),
                Msg::Resize { cols, rows } => {
                    term.resize(cols, rows);
                    force_full = true;
                }
                Msg::RequestFull => force_full = true,
                Msg::Eof | Msg::Shutdown => eof = true,
            }
        }

        for ev in term.drain_events() {
            let mapped = match ev {
                TermEvent::Title(t) => CoreEvent::TitleChanged {
                    session: sid,
                    title: t,
                },
                TermEvent::Cwd(c) => CoreEvent::CwdChanged {
                    session: sid,
                    cwd: c,
                },
                TermEvent::Bell => CoreEvent::Bell { session: sid },
                TermEvent::Hyperlink(_) => continue,
            };
            let _ = events.send(mapped);
        }

        app_cursor.store(term.app_cursor_keys(), Ordering::Relaxed);

        if force_full || term.has_changes() {
            let frame = term.take_frame(sid, force_full);
            force_full = false;
            let _ = events.send(CoreEvent::Frame(frame));
        }

        if eof {
            break;
        }
    }

    let code = pty.lock().try_wait().ok().flatten().unwrap_or(0);
    let _ = events.send(CoreEvent::Exited { session: sid, code });
}
