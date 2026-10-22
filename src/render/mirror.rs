use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::{BLOCK_CHARS, PEAK_CHAR, VisualizerRenderer, VisualizerState, set_cell};

pub struct MirrorRenderer;

impl MirrorRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MirrorRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl VisualizerRenderer for MirrorRenderer {
    fn name(&self) -> &'static str {
        "Mirror Spectrum"
    }

    fn collection(&self) -> &'static str {
        "Retro"
    }

    fn render(&mut self, buf: &mut Buffer, area: Rect, state: &VisualizerState) {
        if area.width < 2 || area.height < 4 || state.values.is_empty() {
            return;
        }

        let width = area.width as usize;
        let height = area.height as usize;
        let center_y = height / 2;
        let half_height = center_y;
        let max_subunits = half_height * 8;

        let val_count = state.values.len();
        let (bar_width, gap) = if width >= val_count * 4 {
            (3, 1)
        } else if width >= val_count * 3 {
            (2, 1)
        } else if width >= (val_count * 3) / 2 {
            (1, 1)
        } else {
            (1, 0)
        };

        let unit_stride = bar_width + gap;
        let max_bars = width / unit_stride.max(1);
        let num_bars = val_count.min(max_bars);

        let used_width = num_bars * unit_stride;
        let x_offset = if width > used_width {
            (width - used_width) / 2
        } else {
            0
        };

        let time = state.elapsed_sec;
        let bass = state.bass_energy;

        // Draw animated liquid centerline with audio-reactive glow
        let center_color = if bass > 0.45 {
            state.palette.accent
        } else {
            state.palette.muted
        };
        let center_char = if bass > 0.6 { '═' } else { '─' };

        for bx in 0..width {
            set_cell(
                buf,
                area,
                area.x as usize + bx,
                area.y as usize + center_y,
                center_char,
                Style::default().fg(center_color),
            );
        }

        for bar_idx in 0..num_bars {
            let val = state.values[bar_idx].clamp(0.0, 1.0);
            let peak = state
                .peaks
                .get(bar_idx)
                .copied()
                .unwrap_or(val)
                .clamp(0.0, 1.0);

            let bar_subunits = ((val * max_subunits as f32).round() as usize).min(max_subunits);
            let peak_subunits = ((peak * max_subunits as f32).round() as usize).min(max_subunits);

            let start_x = area.x as usize + x_offset + bar_idx * unit_stride;

            // 1. Upper half: from center_y - 1 going UP to 0
            for dy in 0..half_height {
                let row_from_center = dy;
                let target_y = center_y - 1 - dy;

                let cell_subunit_start = row_from_center * 8;
                let cell_subunit_end = cell_subunit_start + 8;

                let height_ratio = row_from_center as f32 / half_height.max(1) as f32;
                let cell_color = state.palette.interpolate_gradient(height_ratio);
                let mut cell_style = Style::default().fg(cell_color);

                let (ch, is_tip) = if bar_subunits >= cell_subunit_end {
                    ('█', false)
                } else if bar_subunits > cell_subunit_start {
                    let rem = bar_subunits - cell_subunit_start;
                    (BLOCK_CHARS[rem.min(8)], true)
                } else {
                    (' ', false)
                };

                if is_tip {
                    cell_style = cell_style.add_modifier(Modifier::BOLD);
                }

                let (final_ch, final_style) = if ch == ' '
                    && peak_subunits >= cell_subunit_start
                    && peak_subunits < cell_subunit_end
                    && peak > 0.02
                {
                    let peak_color = if peak > 0.85 {
                        Color::White
                    } else {
                        state.palette.accent
                    };
                    (PEAK_CHAR, Style::default().fg(peak_color).add_modifier(Modifier::BOLD))
                } else {
                    (ch, cell_style)
                };

                if final_ch != ' ' {
                    for bx in 0..bar_width {
                        set_cell(
                            buf,
                            area,
                            start_x + bx,
                            area.y as usize + target_y,
                            final_ch,
                            final_style,
                        );
                    }
                }
            }

            // 2. Lower half: realistic water ripple reflection with depth attenuation
            let remaining_lower = height.saturating_sub(center_y + 1);
            let refl_subunits = (bar_subunits as f32 * 0.80).round() as usize;

            for dy in 0..remaining_lower {
                let row_from_center = dy;
                let target_y = center_y + 1 + dy;

                let cell_subunit_start = row_from_center * 8;
                let cell_subunit_end = cell_subunit_start + 8;

                // Depth fading: deeper reflection becomes progressively darker and more diffused
                let depth_ratio = row_from_center as f32 / remaining_lower.max(1) as f32;
                let orig_color = state.palette.interpolate_gradient(depth_ratio * 0.7);
                let dim_factor = (0.55 - depth_ratio * 0.35).max(0.12);
                let dimmed_color = dim_color(orig_color, dim_factor);
                let cell_style = Style::default().fg(dimmed_color);

                // Animated ripple shimmer on water surface
                let ripple_phase = (bar_idx as f32 * 0.5 + time * 3.5 + dy as f32 * 0.8).sin();
                let shimmer = ripple_phase * bass * 3.0;
                let effective_subunits = (refl_subunits as f32 + shimmer).max(0.0) as usize;

                let ch = if effective_subunits >= cell_subunit_end {
                    if depth_ratio > 0.5 {
                        '▓'
                    } else {
                        '█'
                    }
                } else if effective_subunits > cell_subunit_start {
                    if depth_ratio > 0.6 {
                        '░'
                    } else if depth_ratio > 0.3 {
                        '▒'
                    } else {
                        '▄'
                    }
                } else if depth_ratio < 0.3 && ripple_phase > 0.7 && bass > 0.35 {
                    '~'
                } else {
                    ' '
                };

                if ch != ' ' {
                    for bx in 0..bar_width {
                        set_cell(
                            buf,
                            area,
                            start_x + bx,
                            area.y as usize + target_y,
                            ch,
                            cell_style,
                        );
                    }
                }
            }
        }
    }
}

fn dim_color(color: Color, factor: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            let r = (r as f32 * factor).round() as u8;
            let g = (g as f32 * factor).round() as u8;
            let b = (b as f32 * factor).round() as u8;
            Color::Rgb(r, g, b)
        }
        other => other,
    }
}
