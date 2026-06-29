//! `nova-config` — strongly-typed configuration plus the theme engine.
//!
//! Configuration is JSON. Every field has a `serde` default so a partial (or
//! empty) `config.json` produces a fully-populated [`Config`] by deep-merging
//! over the defaults. Parse failures never crash the app — callers fall back to
//! the last-good config.

#![forbid(unsafe_code)]

mod detect;
mod theme;

pub use detect::detect_profiles;
pub use theme::{builtin_themes, parse_hex_rgba, AnsiColors, Theme, ThemeColors};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to parse config: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Top-level configuration.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub appearance: Appearance,
    pub rendering: Rendering,
    pub terminal: TerminalConfig,
    pub profiles: Profiles,
    pub behavior: Behavior,
}

impl Config {
    /// Parse config from a JSON string, merging over defaults. An empty or
    /// whitespace-only string yields the defaults.
    pub fn from_json(s: &str) -> Result<Config, ConfigError> {
        if s.trim().is_empty() {
            return Ok(Config::default());
        }
        // serde(default) on every field gives us the deep-merge-over-defaults
        // behavior for any present subset of keys.
        Ok(serde_json::from_str(s)?)
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    pub theme: String,
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
    pub ligatures: bool,
    pub cursor: Cursor,
    pub window: Window,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance {
            theme: "tokyo-night".into(),
            font_family: "Cascadia Code".into(),
            font_size: 13.0,
            line_height: 1.2,
            ligatures: true,
            cursor: Cursor::default(),
            window: Window::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Cursor {
    pub style: String,
    pub blink: bool,
}

impl Default for Cursor {
    fn default() -> Self {
        Cursor {
            style: "bar".into(),
            blink: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowMaterial {
    Mica,
    Acrylic,
    Solid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Window {
    pub material: WindowMaterial,
    pub rounded: bool,
    pub padding: [u16; 2],
    pub opacity: f32,
}

impl Default for Window {
    fn default() -> Self {
        Window {
            material: WindowMaterial::Mica,
            rounded: true,
            padding: [8, 8],
            opacity: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rendering {
    /// "auto" | "webgpu" | "webgl" | "canvas"
    pub backend: String,
    pub max_fps: u16,
    pub frame_tick_ms: u16,
}

impl Default for Rendering {
    fn default() -> Self {
        Rendering {
            backend: "auto".into(),
            max_fps: 240,
            frame_tick_ms: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TerminalConfig {
    pub scrollback_lines: u32,
    pub spill_to_disk: bool,
    /// "visual" | "audible" | "none"
    pub bell: String,
    pub copy_on_select: bool,
    pub word_separators: String,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        TerminalConfig {
            scrollback_lines: 100_000,
            spill_to_disk: true,
            bell: "visual".into(),
            copy_on_select: false,
            word_separators: " \t()[]{}\"'".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Profiles {
    pub default: String,
    pub list: Vec<Profile>,
}

impl Default for Profiles {
    fn default() -> Self {
        Profiles {
            default: "pwsh".into(),
            list: vec![
                Profile {
                    id: "pwsh".into(),
                    name: "PowerShell".into(),
                    shell: "pwsh.exe".into(),
                    args: vec!["-NoLogo".into()],
                    icon: None,
                    color: None,
                },
                Profile {
                    id: "powershell".into(),
                    name: "Windows PowerShell".into(),
                    shell: "powershell.exe".into(),
                    args: vec!["-NoLogo".into()],
                    icon: None,
                    color: None,
                },
                Profile {
                    id: "cmd".into(),
                    name: "Command Prompt".into(),
                    shell: "cmd.exe".into(),
                    args: vec![],
                    icon: None,
                    color: None,
                },
                Profile {
                    id: "wsl".into(),
                    name: "WSL".into(),
                    shell: "wsl.exe".into(),
                    args: vec![],
                    icon: None,
                    color: None,
                },
                Profile {
                    id: "gitbash".into(),
                    name: "Git Bash".into(),
                    shell: "C:/Program Files/Git/bin/bash.exe".into(),
                    args: vec!["-i".into(), "-l".into()],
                    icon: None,
                    color: None,
                },
                Profile {
                    id: "nu".into(),
                    name: "Nushell".into(),
                    shell: "nu.exe".into(),
                    args: vec![],
                    icon: None,
                    color: None,
                },
            ],
        }
    }
}

impl Profiles {
    /// Build the profile list by probing the system (see [`detect_profiles`]):
    /// only installed shells, plus one entry per installed WSL distribution.
    /// Falls back to the static default list if detection finds nothing (keeps
    /// the app usable on an unexpected host).
    #[must_use]
    pub fn detected() -> Profiles {
        let list = detect_profiles();
        if list.is_empty() {
            return Profiles::default();
        }
        // Prefer a sensible default that actually exists.
        let default = ["pwsh", "powershell", "cmd"]
            .into_iter()
            .find(|id| list.iter().any(|p| p.id == *id))
            .map(str::to_string)
            .unwrap_or_else(|| list[0].id.clone());
        Profiles { default, list }
    }

    /// Resolve a profile by id, or the configured default, or the first entry.
    pub fn resolve(&self, id: Option<&str>) -> Option<&Profile> {
        let want = id.unwrap_or(&self.default);
        self.list
            .iter()
            .find(|p| p.id == want)
            .or_else(|| self.list.iter().find(|p| p.id == self.default))
            .or_else(|| self.list.first())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub shell: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Behavior {
    pub restore_session: bool,
    pub hibernate_after_min: u32,
    pub confirm_multiline_paste: bool,
}

impl Default for Behavior {
    fn default() -> Self {
        Behavior {
            restore_session: true,
            hibernate_after_min: 15,
            confirm_multiline_paste: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_yields_defaults() {
        let c = Config::from_json("").unwrap();
        assert_eq!(c, Config::default());
    }

    #[test]
    fn partial_merges_over_defaults() {
        let c = Config::from_json(r#"{ "appearance": { "font_size": 16 } }"#).unwrap();
        assert_eq!(c.appearance.font_size, 16.0);
        // untouched fields keep defaults
        assert_eq!(c.appearance.font_family, "Cascadia Code");
        assert_eq!(c.rendering.max_fps, 240);
    }

    #[test]
    fn roundtrips() {
        let c = Config::default();
        let json = c.to_json_pretty();
        let back = Config::from_json(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn profile_resolution() {
        let p = Profiles::default();
        assert_eq!(p.resolve(Some("cmd")).unwrap().shell, "cmd.exe");
        assert_eq!(p.resolve(None).unwrap().id, "pwsh");
        assert_eq!(p.resolve(Some("missing")).unwrap().id, "pwsh");
    }

    #[test]
    fn builtin_themes_present() {
        let themes = builtin_themes();
        let ids: Vec<_> = themes.iter().map(|t| t.id.as_str()).collect();
        for want in ["fluent", "nord", "dracula", "catppuccin", "tokyo-night"] {
            assert!(ids.contains(&want), "missing theme {want}");
        }
    }
}
