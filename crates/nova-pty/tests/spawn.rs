//! Live ConPTY smoke test: spawn a real shell and read its output.
//!
//! Note: a ConPTY keeps its output pipe open until `ClosePseudoConsole` (i.e.
//! until the [`Pty`] is dropped), so a blocking `read` will not see EOF merely
//! because the child exited. We therefore read on a detached thread and poll a
//! shared buffer for a marker with a wall-clock timeout; dropping the `Pty` at
//! end of scope unblocks the reader.
#![cfg(windows)]

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use nova_pty::{CommandBuilder, Pty, PtySize};

/// Spawn a reader thread and poll for `marker` until found or `timeout`.
fn read_until(pty: &mut Pty, marker: &str, timeout: Duration) -> String {
    let mut reader = pty.take_reader().expect("reader");
    let buf = Arc::new(Mutex::new(String::new()));
    let writer_buf = Arc::clone(&buf);
    // Detached: ends when the Pty is dropped and the pipe breaks.
    thread::spawn(move || {
        let mut b = [0u8; 8192];
        loop {
            match reader.read(&mut b) {
                Ok(0) => break,
                Ok(n) => writer_buf
                    .lock()
                    .unwrap()
                    .push_str(&String::from_utf8_lossy(&b[..n])),
                Err(_) => break,
            }
        }
    });

    let deadline = Instant::now() + timeout;
    loop {
        if buf.lock().unwrap().contains(marker) || Instant::now() > deadline {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    let out = buf.lock().unwrap().clone();
    out
}

#[test]
#[ignore = "requires an interactive Windows desktop session; ConPTY/conhost does \
            not relay child output in headless/sandboxed sessions"]
fn spawns_cmd_and_reads_output() {
    let cmd = CommandBuilder::new("cmd.exe").args(["/c", "echo NOVATERM_OK"]);
    let mut pty = Pty::spawn(&cmd, PtySize { cols: 80, rows: 24 }).expect("spawn");
    let out = read_until(&mut pty, "NOVATERM_OK", Duration::from_secs(10));
    assert!(out.contains("NOVATERM_OK"), "missing marker; got:\n{out}");
}

#[test]
#[ignore = "requires an interactive Windows desktop session; ConPTY/conhost does \
            not relay child output in headless/sandboxed sessions"]
fn write_and_resize() {
    let cmd = CommandBuilder::new("cmd.exe").arg("/q");
    let mut pty = Pty::spawn(&cmd, PtySize { cols: 80, rows: 24 }).expect("spawn");
    let mut writer = pty.take_writer().expect("writer");

    pty.resize(PtySize {
        cols: 120,
        rows: 40,
    })
    .expect("resize");
    writer
        .write_all(b"echo SECOND_MARKER\r\nexit\r\n")
        .expect("write");
    writer.flush().ok();

    let out = read_until(&mut pty, "SECOND_MARKER", Duration::from_secs(10));
    assert!(out.contains("SECOND_MARKER"), "got:\n{out}");
    // Pty drops here, closing the pseudoconsole and unblocking the reader thread.
}
