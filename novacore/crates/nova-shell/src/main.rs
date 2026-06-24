//! NovaCore application entry point.
//!
//! A runnable NovaCore shell: it reads NovaLang, evaluates it through the engine
//! (structured values, not text), renders the result, and prints the text view
//! plus a summary of the GPU instances the windowed frontend would upload.
//!
//! The native window is the Phase-1 frontend (`winit` + `wgpu`): it owns the
//! surface/device, builds a pipeline from [`nova_gpu::SHADER`], and each frame
//! uploads the [`nova_gpu::Instance`] buffer produced by [`frame::render_value`]
//! — the exact path exercised here and in the tests. See `ARCHITECTURE.md §7–8`.

mod atlas;
mod frame;
mod gui;

use nova_engine::Engine;
use nova_value::View;
use std::io::{BufRead, Write};

fn main() {
    // GUI by default; `--repl` runs the headless text REPL (used in CI / no display).
    if std::env::args().any(|a| a == "--repl") {
        repl();
    } else {
        gui::run();
    }
}

fn repl() {
    let mut engine = Engine::new();
    let stdout = std::io::stdout();
    let stdin = std::io::stdin();

    println!("NovaCore 0.1 — engine-centric terminal platform");
    println!(
        "Type NovaLang (e.g. `ls | where size > 0 | sort-by name | first 5`). `exit` to quit.\n"
    );

    loop {
        {
            let mut out = stdout.lock();
            let _ = write!(out, "{}❯ ", engine.cwd().display());
            let _ = out.flush();
        }

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(_) => break,
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "exit" || line == "quit" {
            break;
        }

        match engine.eval(line) {
            Ok(value) => {
                // The text view (what a non-GUI context shows).
                let text = value.to_text();
                if !text.is_empty() {
                    println!("{text}");
                }
                // The structured-render pipeline the GUI frontend would draw.
                let (_draw, instances) = frame::render_value(&value, View::Auto, 1200.0, 720.0);
                let glyphs = instances.iter().filter(|i| i.kind == 1).count();
                let rects = instances.len() - glyphs;
                println!(
                    "\x1b[2m[{} type · {} draw instances: {rects} rects, {glyphs} glyphs]\x1b[0m",
                    value.type_name(),
                    instances.len()
                );
            }
            Err(e) => eprintln!("error: {e}"),
        }
    }
}
