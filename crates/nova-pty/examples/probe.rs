//! Manual probe: spawn a shell in a ConPTY and dump exactly what the output
//! pipe delivers (escaped), isolated from any shared console.
//!
//! Run with: `cargo run -p nova-pty --example probe -- powershell.exe`

use std::io::{Read, Write};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use nova_pty::{CommandBuilder, Pty, PtySize};

fn main() {
    let prog = std::env::args().nth(1).unwrap_or_else(|| "cmd.exe".into());
    let cmd = CommandBuilder::new(prog.clone());
    let mut pty = Pty::spawn(&cmd, PtySize { cols: 80, rows: 24 }).expect("spawn");
    eprintln!("[probe] spawned {prog}");

    let mut reader = pty.take_reader().unwrap();
    let mut writer = pty.take_writer().unwrap();

    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        let mut b = [0u8; 8192];
        loop {
            match reader.read(&mut b) {
                Ok(0) => {
                    eprintln!("[probe] reader EOF");
                    break;
                }
                Ok(n) => {
                    if tx.send(b[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("[probe] read err: {e}");
                    break;
                }
            }
        }
    });

    // Give the shell a moment, then send a command.
    thread::sleep(Duration::from_millis(800));
    let _ = writer.write_all(b"echo PROBE_MARKER\r\n");
    let _ = writer.flush();

    let mut total = 0usize;
    let deadline = std::time::Instant::now() + Duration::from_secs(4);
    while std::time::Instant::now() < deadline {
        if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(200)) {
            total += chunk.len();
            let s = String::from_utf8_lossy(&chunk);
            // Escape control chars for readability.
            let esc: String = s
                .chars()
                .map(|c| match c {
                    '\x1b' => "<ESC>".to_string(),
                    '\r' => "<CR>".to_string(),
                    '\n' => "<LF>\n".to_string(),
                    c if (c as u32) < 0x20 => format!("<{:02x}>", c as u32),
                    c => c.to_string(),
                })
                .collect();
            println!("--- chunk {} bytes ---\n{esc}", chunk.len());
        }
    }
    eprintln!("[probe] total bytes from pipe: {total}");
}
