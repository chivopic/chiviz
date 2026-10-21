pub mod classic;
pub mod mirror;
pub mod orbit;
pub mod wave;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::theme::Palette;

pub const BLOCK_CHARS: [char; 9] = [' ', ' ', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
pub const PEAK_CHAR: char = '▔'; // U+2594 Upper one eighth block

pub struct VisualizerState<'a> {
    pub values: &'a [f32],
    pub peaks: &'a [f32],
    pub bass_energy: f32,
    pub mid_energy: f32,
    pub treble_energy: f32,
    pub normalized_energy: f32,
    pub palette: &'a Palette,
    pub elapsed_sec: f32,
    pub is_frozen: bool,
}

pub trait VisualizerRenderer: Send {
    fn render(&mut self, buf: &mut Buffer, area: Rect, state: &VisualizerState);
    #[allow(dead_code)]
    fn name(&self) -> &'static str;
    #[allow(dead_code)]
    fn collection(&self) -> &'static str;
}

#[inline]
pub fn set_cell(
    buf: &mut Buffer,
    area: Rect,
    x: usize,
    y: usize,
    ch: char,
    style: ratatui::style::Style,
) {
    if x >= area.left() as usize
        && x < area.right() as usize
        && y >= area.top() as usize
        && y < area.bottom() as usize
        && let Some(cell) = buf.cell_mut((x as u16, y as u16))
    {
        cell.set_char(ch);
        cell.set_style(style);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualMode {
    Classic,
    Mirror,
    Wave,
    Orbit,
}

impl VisualMode {
    pub fn name(&self) -> &'static str {
        match self {
            VisualMode::Classic => "Classic Bars",
            VisualMode::Mirror => "Mirror Spectrum",
            VisualMode::Wave => "Smooth Wave",
            VisualMode::Orbit => "Orbit",
        }
    }

    pub fn collection(&self) -> &'static str {
        match self {
            VisualMode::Classic | VisualMode::Mirror => "Retro",
            VisualMode::Wave | VisualMode::Orbit => "Modern",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            VisualMode::Classic => VisualMode::Mirror,
            VisualMode::Mirror => VisualMode::Wave,
            VisualMode::Wave => VisualMode::Orbit,
            VisualMode::Orbit => VisualMode::Classic,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            VisualMode::Classic => VisualMode::Orbit,
            VisualMode::Mirror => VisualMode::Classic,
            VisualMode::Wave => VisualMode::Mirror,
            VisualMode::Orbit => VisualMode::Wave,
        }
    }

    pub fn from_number(n: u8) -> Option<Self> {
        match n {
            1 => Some(VisualMode::Classic),
            2 => Some(VisualMode::Mirror),
            3 => Some(VisualMode::Wave),
            4 => Some(VisualMode::Orbit),
            _ => None,
        }
    }

    pub fn from_str_name(s: &str) -> Option<Self> {
        let lower = s.to_lowercase().replace(['-', '_', ' '], "");
        match lower.as_str() {
            "classic" | "bars" | "classicbars" => Some(VisualMode::Classic),
            "mirror" | "mirrorspectrum" => Some(VisualMode::Mirror),
            "wave" | "smoothwave" => Some(VisualMode::Wave),
            "orbit" | "radial" => Some(VisualMode::Orbit),
            _ => None,
        }
    }
}

/// Capability probe for future Kitty Graphics Protocol backend
pub struct KittyGraphicsProbe {
    pub is_kitty: bool,
    pub supports_truecolor: bool,
    pub in_tmux: bool,
}

impl KittyGraphicsProbe {
    pub fn detect() -> Self {
        let term = std::env::var("TERM").unwrap_or_default();
        let kitty_window_id = std::env::var("KITTY_WINDOW_ID").is_ok();
        let kitty_pid = std::env::var("KITTY_PID").is_ok();
        let is_kitty = term.contains("kitty") || kitty_window_id || kitty_pid;

        let colorterm = std::env::var("COLORTERM").unwrap_or_default();
        let supports_truecolor = colorterm == "truecolor" || colorterm == "24bit" || is_kitty;

        let in_tmux = std::env::var("TMUX").is_ok();

        Self {
            is_kitty,
            supports_truecolor,
            in_tmux,
        }
    }
}
