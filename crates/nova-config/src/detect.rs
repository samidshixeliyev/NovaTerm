//! System shell/terminal detection.
//!
//! Builds the *live* list of [`Profile`]s by probing the machine: only shells
//! whose executable actually exists are returned, and every installed WSL
//! distribution becomes its own profile. This is what makes the new-tab menu
//! show every terminal available on the PC — and nothing that isn't installed.
//!
//! Detection is best-effort and never panics: a probe that fails (missing exe,
//! `wsl.exe` erroring, odd console encoding) simply contributes no profile.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Profile;

/// Resolve a bare executable name on the system `PATH`.
fn which(exe: &str) -> Option<PathBuf> {
    if exe.is_empty() {
        return None;
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join(exe);
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

/// Locate a shell: try `PATH` for `exe`, then each absolute fallback in order.
/// Returns the first full path that exists.
fn locate(exe: &str, fallbacks: &[String]) -> Option<String> {
    if let Some(p) = which(exe) {
        return Some(p.to_string_lossy().into_owned());
    }
    fallbacks
        .iter()
        .find(|p| Path::new(p).is_file())
        .cloned()
}

/// Expand a `%VAR%`-style Windows env reference at the head of a path.
fn env_path(var: &str, tail: &str) -> Option<String> {
    let base = std::env::var(var).ok()?;
    Some(format!("{base}{tail}"))
}

fn profile(
    id: &str,
    name: &str,
    shell: String,
    args: &[&str],
    icon: &str,
    color: &str,
) -> Profile {
    Profile {
        id: id.to_string(),
        name: name.to_string(),
        shell,
        args: args.iter().map(|s| s.to_string()).collect(),
        icon: (!icon.is_empty()).then(|| icon.to_string()),
        color: (!color.is_empty()).then(|| color.to_string()),
    }
}

/// Decode the stdout of a Windows console tool. `wsl.exe` emits UTF-16LE;
/// most others emit UTF-8. Heuristic on embedded NULs picks the encoding.
fn decode_console(bytes: &[u8]) -> String {
    let nul_count = bytes.iter().take(64).filter(|&&b| b == 0).count();
    if nul_count >= 2 {
        let u16s: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&u16s)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// Lowercase, hyphenated slug for building stable profile ids.
fn slug(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Enumerate installed WSL distributions, one profile each.
fn detect_wsl(out: &mut Vec<Profile>) {
    let wsl = match locate(
        "wsl.exe",
        &[env_path("SystemRoot", r"\System32\wsl.exe").unwrap_or_default()],
    ) {
        Some(w) => w,
        None => return,
    };

    let output = Command::new(&wsl).args(["--list", "--quiet"]).output();
    let Ok(o) = output else { return };
    if !o.status.success() {
        return;
    }

    for line in decode_console(&o.stdout).lines() {
        let name = line.trim().trim_matches('\u{0}').trim();
        if name.is_empty() {
            continue;
        }
        out.push(Profile {
            id: format!("wsl-{}", slug(name)),
            name: format!("WSL · {name}"),
            shell: wsl.clone(),
            args: vec!["-d".into(), name.to_string()],
            icon: Some("🐧".into()),
            color: Some("#4d8c4d".into()),
        });
    }
}

/// Probe the system and return only the shells/terminals that are installed.
#[must_use]
pub fn detect_profiles() -> Vec<Profile> {
    let mut out: Vec<Profile> = Vec::new();

    // PowerShell 7+ (pwsh).
    if let Some(p) = locate(
        "pwsh.exe",
        &[
            env_path("ProgramFiles", r"\PowerShell\7\pwsh.exe").unwrap_or_default(),
            env_path("ProgramFiles", r"\PowerShell\7-preview\pwsh.exe").unwrap_or_default(),
        ],
    ) {
        out.push(profile("pwsh", "PowerShell", p, &["-NoLogo"], "❯", "#2b579a"));
    }

    // Windows PowerShell (5.1).
    if let Some(p) = locate(
        "powershell.exe",
        &[env_path("SystemRoot", r"\System32\WindowsPowerShell\v1.0\powershell.exe")
            .unwrap_or_default()],
    ) {
        out.push(profile(
            "powershell",
            "Windows PowerShell",
            p,
            &["-NoLogo"],
            "❯",
            "#012456",
        ));
    }

    // Command Prompt.
    if let Some(p) = locate(
        "cmd.exe",
        &[env_path("SystemRoot", r"\System32\cmd.exe").unwrap_or_default()],
    ) {
        out.push(profile("cmd", "Command Prompt", p, &[], "▶", "#0c0c0c"));
    }

    // Git Bash (not on PATH by default — probe known install dirs).
    if let Some(p) = locate(
        "",
        &[
            r"C:\Program Files\Git\bin\bash.exe".to_string(),
            r"C:\Program Files (x86)\Git\bin\bash.exe".to_string(),
            env_path("LOCALAPPDATA", r"\Programs\Git\bin\bash.exe").unwrap_or_default(),
        ],
    ) {
        out.push(profile(
            "gitbash",
            "Git Bash",
            p,
            &["-i", "-l"],
            "",
            "#dd4814",
        ));
    }

    // Nushell.
    if let Some(p) = locate(
        "nu.exe",
        &[env_path("USERPROFILE", r"\.cargo\bin\nu.exe").unwrap_or_default()],
    ) {
        out.push(profile("nu", "Nushell", p, &[], "", "#3aa675"));
    }

    // MSYS2 (UCRT64 login shell).
    if let Some(p) = locate(
        "",
        &[
            r"C:\msys64\usr\bin\bash.exe".to_string(),
            r"C:\msys32\usr\bin\bash.exe".to_string(),
        ],
    ) {
        out.push(profile("msys2", "MSYS2", p, &["--login", "-i"], "", "#5a8f29"));
    }

    // Cygwin.
    if let Some(p) = locate(
        "",
        &[
            r"C:\cygwin64\bin\bash.exe".to_string(),
            r"C:\cygwin\bin\bash.exe".to_string(),
        ],
    ) {
        out.push(profile("cygwin", "Cygwin", p, &["-i", "-l"], "", "#6aab3a"));
    }

    // Every installed WSL distribution.
    detect_wsl(&mut out);

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_stable() {
        assert_eq!(slug("Ubuntu-22.04"), "ubuntu-22-04");
        assert_eq!(slug("  Debian  "), "debian");
        assert_eq!(slug("openSUSE Leap"), "opensuse-leap");
    }

    #[test]
    fn decodes_utf16le_console() {
        // "Ubuntu\n" as UTF-16LE.
        let mut bytes = Vec::new();
        for ch in "Ubuntu\n".chars() {
            bytes.extend_from_slice(&(ch as u16).to_le_bytes());
        }
        assert_eq!(decode_console(&bytes).trim(), "Ubuntu");
    }

    #[test]
    fn decodes_utf8_console() {
        assert_eq!(decode_console(b"Ubuntu\n").trim(), "Ubuntu");
    }

    #[test]
    fn detection_never_panics() {
        // Whatever the host has, this must return without panicking.
        let _ = detect_profiles();
    }
}
