pub mod terminal;

use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::audio::SourceStatus;
use crate::render::{VisualMode, VisualizerRenderer, VisualizerState};
use crate::theme::ThemeManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutPreference {
    Auto,
    Compact,
    Immersive,
}

impl LayoutPreference {
    pub fn name(&self) -> &'static str {
        match self {
            LayoutPreference::Auto => "Auto",
            LayoutPreference::Compact => "Compact",
            LayoutPreference::Immersive => "Immersive",
        }
    }

    pub fn from_str_name(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "auto" => Some(LayoutPreference::Auto),
            "compact" | "tiled" => Some(LayoutPreference::Compact),
            "immersive" | "fullscreen" => Some(LayoutPreference::Immersive),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub created_at: Instant,
    pub duration: Duration,
}

pub struct UiState {
    pub layout_pref: LayoutPreference,
    pub immersive_override: Option<bool>,
    pub show_help: bool,
    pub fps: f32,
    frame_count: u32,
    last_fps_calc: Instant,
    pub toast: Option<Toast>,
}

impl UiState {
    pub fn new(layout_pref: LayoutPreference) -> Self {
        Self {
            layout_pref,
            immersive_override: None,
            show_help: false,
            fps: 60.0,
            frame_count: 0,
            last_fps_calc: Instant::now(),
            toast: None,
        }
    }

    pub fn show_toast<S: Into<String>>(&mut self, message: S) {
        self.toast = Some(Toast {
            message: message.into(),
            created_at: Instant::now(),
            duration: Duration::from_millis(1800),
        });
    }

    pub fn active_toast(&self) -> Option<&str> {
        if let Some(t) = &self.toast {
            if t.created_at.elapsed() < t.duration {
                return Some(&t.message);
            }
        }
        None
    }

    pub fn tick_fps(&mut self) {
        self.frame_count += 1;
        let elapsed = self.last_fps_calc.elapsed().as_secs_f32();
        if elapsed >= 0.5 {
            self.fps = (self.frame_count as f32 / elapsed).round();
            self.frame_count = 0;
            self.last_fps_calc = Instant::now();
        }
    }

    pub fn toggle_immersive(&mut self, is_currently_immersive: bool) {
        self.immersive_override = Some(!is_currently_immersive);
    }

    pub fn is_immersive(&self, width: u16, height: u16) -> bool {
        if let Some(forced) = self.immersive_override {
            return forced;
        }
        match self.layout_pref {
            LayoutPreference::Immersive => true,
            LayoutPreference::Compact => false,
            LayoutPreference::Auto => width >= 80 && height >= 22,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_frame(
    buf: &mut Buffer,
    area: Rect,
    renderer: &mut dyn VisualizerRenderer,
    current_mode: VisualMode,
    theme_mgr: &ThemeManager,
    ui_state: &mut UiState,
    source_status: &SourceStatus,
    state: &VisualizerState,
    sensitivity: f32,
) {
    ui_state.tick_fps();
    let palette = theme_mgr.current_palette();
    let is_immersive = ui_state.is_immersive(area.width, area.height);

    if is_immersive {
        // Immersive layout: visualizer fills entire area
        renderer.render(buf, area, state);

        // Minimal floating header badge in top-left
        if area.height >= 4 && area.width >= 35 {
            let badge_text = format!(
                " chiviz • {} • {} • {:.0} FPS ",
                current_mode.name(),
                theme_mgr.current_mode.name(),
                ui_state.fps
            );
            let badge_style = Style::default()
                .fg(palette.foreground)
                .bg(palette.background)
                .add_modifier(Modifier::DIM);

            for (i, ch) in badge_text.chars().enumerate() {
                crate::render::set_cell(
                    buf,
                    area,
                    (area.x + 1 + i as u16) as usize,
                    area.y as usize,
                    ch,
                    badge_style,
                );
            }
        }
    } else {
        // Compact / Tiled layout
        if area.height <= 5 || area.width <= 25 {
            // Ultra-compact: dedicate all rows to visualizer
            renderer.render(buf, area, state);
        } else {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(2)])
                .split(area);

            // Render 1-line compact status bar
            render_compact_status_bar(
                buf,
                chunks[0],
                current_mode,
                theme_mgr,
                source_status,
                ui_state.fps,
                sensitivity,
                state.is_frozen,
            );

            // Render visualizer
            renderer.render(buf, chunks[1], state);
        }
    }

    // Render top floating toast notification if active
    if let Some(msg) = ui_state.active_toast() {
        render_top_toast(buf, area, msg, &palette);
    }

    // Render help overlay if active
    if ui_state.show_help {
        render_help_overlay(
            buf,
            area,
            &palette,
            current_mode,
            theme_mgr,
            source_status,
            ui_state,
            sensitivity,
        );
    }
}

fn render_top_toast(buf: &mut Buffer, area: Rect, msg: &str, palette: &crate::theme::Palette) {
    if area.width < 12 || area.height < 2 {
        return;
    }

    let char_count = msg.chars().count();
    let card_width = ((char_count + 6) as u16)
        .clamp(20, area.width.saturating_sub(4).max(12));
    let card_x = area.x + (area.width.saturating_sub(card_width)) / 2;

    if area.height >= 7 {
        let toast_area = Rect {
            x: card_x,
            y: area.y + 1,
            width: card_width,
            height: 3,
        };
        Clear.render(toast_area, buf);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.accent))
            .style(Style::default().bg(palette.background));
        let text = Paragraph::new(Line::from(vec![
            Span::styled(
                msg,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .alignment(Alignment::Center)
        .block(block);
        text.render(toast_area, buf);
    } else {
        let toast_area = Rect {
            x: card_x,
            y: area.y,
            width: card_width,
            height: 1,
        };
        Clear.render(toast_area, buf);
        let pill = Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {} ", msg),
                Style::default()
                    .fg(palette.background)
                    .bg(palette.accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .alignment(Alignment::Center);
        pill.render(toast_area, buf);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_compact_status_bar(
    buf: &mut Buffer,
    area: Rect,
    mode: VisualMode,
    theme_mgr: &ThemeManager,
    source_status: &SourceStatus,
    fps: f32,
    sensitivity: f32,
    frozen: bool,
) {
    let palette = theme_mgr.current_palette();
    let theme_label = match theme_mgr.current_mode {
        crate::theme::ThemeMode::OmarchyAuto => {
            format!("Omarchy [{}]", theme_mgr.omarchy_theme_name)
        }
        ref tm => tm.name().to_string(),
    };

    let status_str = match source_status {
        SourceStatus::Active(dev) => dev.as_str(),
        SourceStatus::Idle(dev) => dev.as_str(),
        SourceStatus::Reconnecting(msg) => msg.as_str(),
        SourceStatus::Error(err) => err.as_str(),
    };
    let status_color = if source_status.is_active() {
        palette.accent
    } else {
        palette.yellow
    };

    let freeze_indicator = if frozen { " [PAUSED]" } else { "" };

    let line = Line::from(vec![
        Span::styled(
            " ⚡ chiviz ",
            Style::default()
                .fg(palette.background)
                .bg(palette.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            mode.name(),
            Style::default()
                .fg(palette.foreground)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" • ", Style::default().fg(palette.muted)),
        Span::styled(&theme_label, Style::default().fg(palette.cyan)),
        Span::styled(" • ", Style::default().fg(palette.muted)),
        Span::styled(status_str, Style::default().fg(status_color)),
        Span::styled(" • ", Style::default().fg(palette.muted)),
        Span::styled(
            format!("{:.1}x", sensitivity),
            Style::default().fg(palette.yellow),
        ),
        Span::styled(" • ", Style::default().fg(palette.muted)),
        Span::styled(
            format!("{:.0} FPS", fps),
            Style::default().fg(palette.green),
        ),
        Span::styled(
            freeze_indicator,
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "  (? for help)",
            Style::default()
                .fg(palette.muted)
                .add_modifier(Modifier::DIM),
        ),
    ]);

    let p = Paragraph::new(line);
    p.render(area, buf);
}

#[allow(clippy::too_many_arguments)]
fn render_help_overlay(
    buf: &mut Buffer,
    area: Rect,
    palette: &crate::theme::Palette,
    mode: VisualMode,
    theme_mgr: &ThemeManager,
    source_status: &SourceStatus,
    ui_state: &UiState,
    sensitivity: f32,
) {
    let modal_width = (area.width.saturating_sub(4)).min(62);
    let modal_height = (area.height.saturating_sub(2)).min(17);

    if modal_width < 30 || modal_height < 10 {
        return;
    }

    let modal_x = area.x + (area.width - modal_width) / 2;
    let modal_y = area.y + (area.height - modal_height) / 2;
    let modal_rect = Rect::new(modal_x, modal_y, modal_width, modal_height);

    // Clear background
    Clear.render(modal_rect, buf);

    let border_style = Style::default().fg(palette.accent);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" ⚡ chiviz — Omarchy Audio Visualizer ")
        .title_alignment(Alignment::Center);

    let inner = block.inner(modal_rect);
    block.render(modal_rect, buf);

    let key_style = Style::default()
        .fg(palette.accent)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(palette.foreground);
    let dim_style = Style::default()
        .fg(palette.muted)
        .add_modifier(Modifier::DIM);

    let text = vec![
        Line::from(vec![
            Span::styled(" Space / m  ", key_style),
            Span::styled("Cycle next visualization mode", desc_style),
        ]),
        Line::from(vec![
            Span::styled(" 1–4        ", key_style),
            Span::styled("Direct mode (Classic, Mirror, Wave, Orbit)", desc_style),
        ]),
        Line::from(vec![
            Span::styled(" Tab        ", key_style),
            Span::styled("Cycle next theme (Shift+Tab for prev)", desc_style),
        ]),
        Line::from(vec![
            Span::styled(" f          ", key_style),
            Span::styled("Toggle immersive / compact layout", desc_style),
        ]),
        Line::from(vec![
            Span::styled(" + / -      ", key_style),
            Span::styled("Adjust sensitivity", desc_style),
        ]),
        Line::from(vec![
            Span::styled(" s          ", key_style),
            Span::styled("Freeze / resume visualization", desc_style),
        ]),
        Line::from(vec![
            Span::styled(" ? / h      ", key_style),
            Span::styled("Toggle this help dialog", desc_style),
        ]),
        Line::from(vec![
            Span::styled(" q / Esc    ", key_style),
            Span::styled("Quit", desc_style),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Status: ", dim_style),
            Span::styled(
                format!(
                    "{} ({}) | {} | {} Layout | {:.1}x Sens | {:.0} FPS",
                    mode.name(),
                    mode.collection(),
                    theme_mgr.current_mode.name(),
                    ui_state.layout_pref.name(),
                    sensitivity,
                    ui_state.fps
                ),
                Style::default().fg(palette.cyan),
            ),
        ]),
        Line::from(vec![
            Span::styled("Sink:   ", dim_style),
            Span::styled(source_status.label(), Style::default().fg(palette.yellow)),
        ]),
        Line::from(""),
        Line::from(Span::styled(" [Press ? or Esc to close] ", dim_style)),
    ];

    let p = Paragraph::new(text);
    p.render(inner, buf);
}
