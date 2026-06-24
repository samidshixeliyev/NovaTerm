//! Translate logical key events into the byte sequences a PTY expects,
//! honoring DEC application-cursor-key mode and modifier encodings.

use nova_protocol::{KeyEvent, KeyModifiers};

/// Encode a key event into bytes to write to the PTY. `app_cursor` selects
/// `ESC O x` vs `ESC [ x` for the arrow/navigation keys.
#[must_use]
pub fn encode_key(ev: &KeyEvent, app_cursor: bool) -> Vec<u8> {
    let m = &ev.mods;

    // Named keys first.
    match ev.key.as_str() {
        "Enter" => return b"\r".to_vec(),
        "Tab" => {
            return if m.shift {
                b"\x1b[Z".to_vec()
            } else {
                b"\t".to_vec()
            }
        }
        "Backspace" => {
            return if m.alt {
                b"\x1b\x7f".to_vec()
            } else {
                b"\x7f".to_vec()
            }
        }
        "Escape" => return b"\x1b".to_vec(),
        "ArrowUp" => return cursor_key(b'A', m, app_cursor),
        "ArrowDown" => return cursor_key(b'B', m, app_cursor),
        "ArrowRight" => return cursor_key(b'C', m, app_cursor),
        "ArrowLeft" => return cursor_key(b'D', m, app_cursor),
        "Home" => return cursor_key(b'H', m, app_cursor),
        "End" => return cursor_key(b'F', m, app_cursor),
        "Insert" => return tilde_key(2, m),
        "Delete" => return tilde_key(3, m),
        "PageUp" => return tilde_key(5, m),
        "PageDown" => return tilde_key(6, m),
        "F1" => return ss3(b'P'),
        "F2" => return ss3(b'Q'),
        "F3" => return ss3(b'R'),
        "F4" => return ss3(b'S'),
        "F5" => return tilde_key(15, m),
        "F6" => return tilde_key(17, m),
        "F7" => return tilde_key(18, m),
        "F8" => return tilde_key(19, m),
        "F9" => return tilde_key(20, m),
        "F10" => return tilde_key(21, m),
        "F11" => return tilde_key(23, m),
        "F12" => return tilde_key(24, m),
        _ => {}
    }

    // Printable / character input.
    let ch = if let Some(text) = &ev.text {
        text.clone()
    } else if ev.key.chars().count() == 1 {
        ev.key.clone()
    } else {
        return Vec::new();
    };

    // Ctrl+<letter> and a few ctrl symbols → control codes.
    if m.ctrl && !m.alt {
        if let Some(b) = ctrl_byte(&ch) {
            return vec![b];
        }
    }

    let mut out = Vec::new();
    if m.alt {
        out.push(0x1b); // Alt = ESC prefix
    }
    out.extend_from_slice(ch.as_bytes());
    out
}

/// Modifier parameter per the xterm `1 + bitmask` convention.
fn modifier_param(m: &KeyModifiers) -> Option<u8> {
    let mut bits = 0;
    if m.shift {
        bits |= 1;
    }
    if m.alt {
        bits |= 2;
    }
    if m.ctrl {
        bits |= 4;
    }
    if bits == 0 {
        None
    } else {
        Some(bits + 1)
    }
}

fn cursor_key(final_byte: u8, m: &KeyModifiers, app_cursor: bool) -> Vec<u8> {
    if let Some(p) = modifier_param(m) {
        // CSI 1 ; <mod> <final>
        return format!("\x1b[1;{}{}", p, final_byte as char).into_bytes();
    }
    if app_cursor {
        vec![0x1b, b'O', final_byte]
    } else {
        vec![0x1b, b'[', final_byte]
    }
}

fn tilde_key(n: u8, m: &KeyModifiers) -> Vec<u8> {
    if let Some(p) = modifier_param(m) {
        format!("\x1b[{n};{p}~").into_bytes()
    } else {
        format!("\x1b[{n}~").into_bytes()
    }
}

fn ss3(b: u8) -> Vec<u8> {
    vec![0x1b, b'O', b]
}

/// Map a single character to its Ctrl control code (Ctrl+A => 0x01, etc.).
fn ctrl_byte(ch: &str) -> Option<u8> {
    let c = ch.chars().next()?;
    match c {
        'a'..='z' => Some((c as u8) - b'a' + 1),
        'A'..='Z' => Some((c as u8) - b'A' + 1),
        ' ' | '@' => Some(0),
        '[' => Some(0x1b),
        '\\' => Some(0x1c),
        ']' => Some(0x1d),
        '^' => Some(0x1e),
        '_' => Some(0x1f),
        '?' => Some(0x7f),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(k: &str) -> KeyEvent {
        KeyEvent {
            key: k.into(),
            mods: KeyModifiers::default(),
            text: None,
        }
    }

    #[test]
    fn enter_and_backspace() {
        assert_eq!(encode_key(&key("Enter"), false), b"\r");
        assert_eq!(encode_key(&key("Backspace"), false), b"\x7f");
    }

    #[test]
    fn arrows_respect_app_mode() {
        assert_eq!(encode_key(&key("ArrowUp"), false), b"\x1b[A");
        assert_eq!(encode_key(&key("ArrowUp"), true), b"\x1bOA");
    }

    #[test]
    fn modified_arrow() {
        let mut k = key("ArrowRight");
        k.mods.ctrl = true;
        assert_eq!(encode_key(&k, false), b"\x1b[1;5C");
    }

    #[test]
    fn ctrl_c() {
        let mut k = key("c");
        k.mods.ctrl = true;
        assert_eq!(encode_key(&k, false), vec![0x03]);
    }

    #[test]
    fn alt_prefixes_escape() {
        let mut k = key("x");
        k.mods.alt = true;
        assert_eq!(encode_key(&k, false), vec![0x1b, b'x']);
    }

    #[test]
    fn plain_text() {
        assert_eq!(encode_key(&key("a"), false), b"a");
    }

    #[test]
    fn function_keys() {
        assert_eq!(encode_key(&key("F1"), false), b"\x1bOP");
        assert_eq!(encode_key(&key("F5"), false), b"\x1b[15~");
    }
}
