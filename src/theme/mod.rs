use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use ratatui::style::Color;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    pub background: Color,
    pub foreground: Color,
    pub accent: Color,
    pub muted: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,
    pub gradient: Vec<Color>,
}

impl Palette {
    /// Linearly interpolate color along the gradient for ratio t in [0.0, 1.0]
    pub fn interpolate_gradient(&self, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        if self.gradient.is_empty() {
            return self.accent;
        }
        if self.gradient.len() == 1 {
            return self.gradient[0];
        }

        let num_segments = (self.gradient.len() - 1) as f32;
        let scaled = t * num_segments;
        let idx = (scaled.floor() as usize).min(self.gradient.len() - 2);
        let frac = scaled - idx as f32;

        let c1 = self.gradient[idx];
        let c2 = self.gradient[idx + 1];

        match (c1, c2) {
            (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
                let r = (r1 as f32 * (1.0 - frac) + r2 as f32 * frac).round() as u8;
                let g = (g1 as f32 * (1.0 - frac) + g2 as f32 * frac).round() as u8;
                let b = (b1 as f32 * (1.0 - frac) + b2 as f32 * frac).round() as u8;
                Color::Rgb(r, g, b)
            }
            _ => c2,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ThemeMode {
    OmarchyAuto,
    AutoAccent,
    AutoMonochrome,
    TokyoNight,
    Catppuccin,
    NeonSynth,
    WinampRetro,
}

impl ThemeMode {
    pub fn name(&self) -> &'static str {
        match self {
            ThemeMode::OmarchyAuto => "Omarchy Auto",
            ThemeMode::AutoAccent => "Auto Accent",
            ThemeMode::AutoMonochrome => "Auto Monochrome",
            ThemeMode::TokyoNight => "Tokyo Night",
            ThemeMode::Catppuccin => "Catppuccin Mocha",
            ThemeMode::NeonSynth => "Neon Synth",
            ThemeMode::WinampRetro => "Winamp Retro",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            ThemeMode::OmarchyAuto => ThemeMode::AutoAccent,
            ThemeMode::AutoAccent => ThemeMode::AutoMonochrome,
            ThemeMode::AutoMonochrome => ThemeMode::TokyoNight,
            ThemeMode::TokyoNight => ThemeMode::Catppuccin,
            ThemeMode::Catppuccin => ThemeMode::NeonSynth,
            ThemeMode::NeonSynth => ThemeMode::WinampRetro,
            ThemeMode::WinampRetro => ThemeMode::OmarchyAuto,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            ThemeMode::OmarchyAuto => ThemeMode::WinampRetro,
            ThemeMode::AutoAccent => ThemeMode::OmarchyAuto,
            ThemeMode::AutoMonochrome => ThemeMode::AutoAccent,
            ThemeMode::TokyoNight => ThemeMode::AutoMonochrome,
            ThemeMode::Catppuccin => ThemeMode::TokyoNight,
            ThemeMode::NeonSynth => ThemeMode::Catppuccin,
            ThemeMode::WinampRetro => ThemeMode::NeonSynth,
        }
    }

    pub fn from_str_name(s: &str) -> Option<Self> {
        let lower = s.to_lowercase().replace(['-', '_', ' '], "");
        match lower.as_str() {
            "auto" | "omarchy" | "omarchyauto" => Some(ThemeMode::OmarchyAuto),
            "accent" | "autoaccent" => Some(ThemeMode::AutoAccent),
            "mono" | "monochrome" | "automonochrome" => Some(ThemeMode::AutoMonochrome),
            "tokyonight" | "tokyo" => Some(ThemeMode::TokyoNight),
            "catppuccin" | "mocha" => Some(ThemeMode::Catppuccin),
            "neon" | "neonsynth" => Some(ThemeMode::NeonSynth),
            "winamp" | "retro" | "winampretro" => Some(ThemeMode::WinampRetro),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct OmarchyRawColors {
    accent: Option<String>,
    muted: Option<String>,
    background: Option<String>,
    foreground: Option<String>,
    red: Option<String>,
    yellow: Option<String>,
    green: Option<String>,
    cyan: Option<String>,
    blue: Option<String>,
    magenta: Option<String>,
}

pub struct ThemeManager {
    pub current_mode: ThemeMode,
    pub omarchy_palette: Option<Palette>,
    pub omarchy_theme_name: String,
    colors_file_path: PathBuf,
    last_modified: Option<SystemTime>,
}

impl ThemeManager {
    pub fn new(mode: ThemeMode) -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/chiv".to_string());
        let colors_path =
            PathBuf::from(home).join(".local/state/omarchy/current/theme/colors.toml");

        let mut mgr = Self {
            current_mode: mode,
            omarchy_palette: None,
            omarchy_theme_name: "Default".to_string(),
            colors_file_path: colors_path,
            last_modified: None,
        };

        mgr.reload_omarchy();
        mgr
    }

    /// Check if the omarchy colors file or symlink has changed and reload
    pub fn check_for_updates(&mut self) -> bool {
        if let Ok(metadata) = fs::metadata(&self.colors_file_path)
            && let Ok(mtime) = metadata.modified()
            && (self.last_modified.is_none() || Some(mtime) != self.last_modified)
        {
            self.last_modified = Some(mtime);
            self.reload_omarchy();
            return true;
        }
        false
    }

    pub fn reload_omarchy(&mut self) {
        if let Ok(content) = fs::read_to_string(&self.colors_file_path)
            && let Ok(parsed) = toml::from_str::<OmarchyRawColors>(&content)
        {
            let bg = parse_hex_color(parsed.background.as_deref().unwrap_or("#000000"));
            let fg = parse_hex_color(parsed.foreground.as_deref().unwrap_or("#ffffff"));
            let accent = parse_hex_color(parsed.accent.as_deref().unwrap_or("#8d8d8d"));
            let muted = parse_hex_color(parsed.muted.as_deref().unwrap_or("#7a7a7a"));

            let red = parse_hex_color(parsed.red.as_deref().unwrap_or("#ff5555"));
            let yellow = parse_hex_color(parsed.yellow.as_deref().unwrap_or("#f1fa8c"));
            let green = parse_hex_color(parsed.green.as_deref().unwrap_or("#50fa7b"));
            let cyan = parse_hex_color(parsed.cyan.as_deref().unwrap_or("#8be9fd"));
            let blue = parse_hex_color(parsed.blue.as_deref().unwrap_or("#bd93f9"));
            let magenta = parse_hex_color(parsed.magenta.as_deref().unwrap_or("#ff79c6"));
            let is_monochrome = is_rgb_grey(red) && is_rgb_grey(green) && is_rgb_grey(blue);
            let gradient = if is_monochrome {
                vec![
                    Color::Rgb(70, 70, 75),
                    Color::Rgb(110, 110, 120),
                    Color::Rgb(165, 165, 180),
                    Color::Rgb(215, 215, 230),
                    Color::Rgb(255, 255, 255),
                ]
            } else {
                vec![blue, cyan, green, yellow, accent, red]
            };
            let active_accent = if is_monochrome {
                Color::Rgb(255, 255, 255)
            } else {
                accent
            };

            self.omarchy_palette = Some(Palette {
                background: bg,
                foreground: fg,
                accent: active_accent,
                muted,
                red,
                yellow,
                green,
                cyan,
                blue,
                magenta,
                gradient,
            });
        }

        // Try reading theme name
        let theme_name_path = self
            .colors_file_path
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("theme.name"));
        if let Some(path) = theme_name_path
            && let Ok(name) = fs::read_to_string(path)
        {
            self.omarchy_theme_name = name.trim().to_string();
        }
    }

    pub fn current_palette(&self) -> Palette {
        match self.current_mode {
            ThemeMode::OmarchyAuto => {
                if let Some(pal) = &self.omarchy_palette {
                    pal.clone()
                } else {
                    Self::preset_tokyo_night()
                }
            }
            ThemeMode::AutoAccent => {
                let base = self
                    .omarchy_palette
                    .clone()
                    .unwrap_or_else(Self::preset_catppuccin);
                Palette {
                    gradient: vec![base.muted, base.blue, base.accent, base.foreground],
                    ..base
                }
            }
            ThemeMode::AutoMonochrome => {
                let base = self
                    .omarchy_palette
                    .clone()
                    .unwrap_or_else(Self::preset_tokyo_night);
                Palette {
                    gradient: vec![
                        Color::Rgb(40, 40, 40),
                        Color::Rgb(90, 90, 90),
                        Color::Rgb(150, 150, 150),
                        Color::Rgb(210, 210, 210),
                        Color::Rgb(255, 255, 255),
                    ],
                    accent: Color::Rgb(200, 200, 200),
                    ..base
                }
            }
            ThemeMode::TokyoNight => Self::preset_tokyo_night(),
            ThemeMode::Catppuccin => Self::preset_catppuccin(),
            ThemeMode::NeonSynth => Self::preset_neon_synth(),
            ThemeMode::WinampRetro => Self::preset_winamp_retro(),
        }
    }

    pub fn preset_tokyo_night() -> Palette {
        Palette {
            background: Color::Rgb(26, 27, 38),    // #1a1b26
            foreground: Color::Rgb(192, 202, 245), // #c0caf5
            accent: Color::Rgb(122, 162, 247),     // #7aa2f7
            muted: Color::Rgb(86, 95, 137),        // #565f89
            red: Color::Rgb(247, 118, 142),        // #f7768e
            yellow: Color::Rgb(224, 175, 104),     // #e0af68
            green: Color::Rgb(158, 206, 106),      // #9ece6a
            cyan: Color::Rgb(125, 207, 255),       // #7dcfff
            blue: Color::Rgb(122, 162, 247),       // #7aa2f7
            magenta: Color::Rgb(187, 154, 247),    // #bb9af7
            gradient: vec![
                Color::Rgb(122, 162, 247),
                Color::Rgb(125, 207, 255),
                Color::Rgb(158, 206, 106),
                Color::Rgb(224, 175, 104),
                Color::Rgb(247, 118, 142),
            ],
        }
    }

    pub fn preset_catppuccin() -> Palette {
        Palette {
            background: Color::Rgb(30, 30, 46),    // #1e1e2e
            foreground: Color::Rgb(205, 214, 244), // #cdd6f4
            accent: Color::Rgb(203, 166, 247),     // #cba6f7
            muted: Color::Rgb(108, 112, 134),      // #6c7086
            red: Color::Rgb(243, 139, 168),        // #f38ba8
            yellow: Color::Rgb(249, 226, 175),     // #f9e2af
            green: Color::Rgb(166, 227, 161),      // #a6e3a1
            cyan: Color::Rgb(148, 226, 213),       // #94e2d5
            blue: Color::Rgb(137, 180, 250),       // #89b4fa
            magenta: Color::Rgb(245, 194, 231),    // #f5c2e7
            gradient: vec![
                Color::Rgb(137, 180, 250),
                Color::Rgb(148, 226, 213),
                Color::Rgb(166, 227, 161),
                Color::Rgb(249, 226, 175),
                Color::Rgb(243, 139, 168),
            ],
        }
    }

    pub fn preset_neon_synth() -> Palette {
        Palette {
            background: Color::Rgb(13, 2, 33), // #0d0221
            foreground: Color::Rgb(255, 255, 255),
            accent: Color::Rgb(0, 240, 255), // #00f0ff
            muted: Color::Rgb(80, 40, 100),
            red: Color::Rgb(255, 0, 127),     // #ff007f
            yellow: Color::Rgb(255, 230, 0),  // #ffe600
            green: Color::Rgb(56, 239, 125),  // #38ef7d
            cyan: Color::Rgb(0, 240, 255),    // #00f0ff
            blue: Color::Rgb(121, 40, 202),   // #7928ca
            magenta: Color::Rgb(255, 0, 127), // #ff007f
            gradient: vec![
                Color::Rgb(121, 40, 202), // Purple
                Color::Rgb(0, 240, 255),  // Cyan
                Color::Rgb(56, 239, 125), // Electric green
                Color::Rgb(255, 230, 0),  // Bright yellow
                Color::Rgb(255, 0, 127),  // Neon pink
            ],
        }
    }

    pub fn preset_winamp_retro() -> Palette {
        Palette {
            background: Color::Rgb(0, 0, 0),
            foreground: Color::Rgb(0, 255, 0), // Classic terminal green
            accent: Color::Rgb(0, 255, 0),
            muted: Color::Rgb(40, 80, 40),
            red: Color::Rgb(255, 0, 0),
            yellow: Color::Rgb(255, 255, 0),
            green: Color::Rgb(0, 255, 0),
            cyan: Color::Rgb(0, 255, 255),
            blue: Color::Rgb(0, 100, 255),
            magenta: Color::Rgb(255, 0, 255),
            gradient: vec![
                Color::Rgb(0, 255, 0),   // Green
                Color::Rgb(128, 255, 0), // Lime
                Color::Rgb(255, 255, 0), // Yellow
                Color::Rgb(255, 128, 0), // Orange
                Color::Rgb(255, 0, 0),   // Red
            ],
        }
    }
}

pub fn is_rgb_grey(c: Color) -> bool {
    match c {
        Color::Rgb(r, g, b) => (r as i32 - g as i32).abs() < 12 && (g as i32 - b as i32).abs() < 12,
        _ => false,
    }
}

pub fn parse_hex_color(hex: &str) -> Color {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() == 6
        && let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&clean[0..2], 16),
            u8::from_str_radix(&clean[2..4], 16),
            u8::from_str_radix(&clean[4..6], 16),
        )
    {
        return Color::Rgb(r, g, b);
    } else if clean.len() == 3
        && let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&clean[0..1].repeat(2), 16),
            u8::from_str_radix(&clean[1..2].repeat(2), 16),
            u8::from_str_radix(&clean[2..3].repeat(2), 16),
        )
    {
        return Color::Rgb(r, g, b);
    }
    Color::White
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color() {
        assert_eq!(parse_hex_color("#ff0000"), Color::Rgb(255, 0, 0));
        assert_eq!(parse_hex_color("#00ff00"), Color::Rgb(0, 255, 0));
        assert_eq!(parse_hex_color("#1a1b26"), Color::Rgb(26, 27, 38));
        assert_eq!(parse_hex_color("#fff"), Color::Rgb(255, 255, 255));
    }

    #[test]
    fn test_palette_gradient_interpolation() {
        let p = Palette {
            background: Color::Black,
            foreground: Color::White,
            accent: Color::Cyan,
            muted: Color::DarkGray,
            red: Color::Red,
            green: Color::Green,
            yellow: Color::Yellow,
            blue: Color::Blue,
            magenta: Color::Magenta,
            cyan: Color::Cyan,
            gradient: vec![Color::Rgb(0, 0, 0), Color::Rgb(100, 100, 100)],
        };

        assert_eq!(p.interpolate_gradient(0.0), Color::Rgb(0, 0, 0));
        assert_eq!(p.interpolate_gradient(1.0), Color::Rgb(100, 100, 100));
        assert_eq!(p.interpolate_gradient(0.5), Color::Rgb(50, 50, 50));
    }
}
