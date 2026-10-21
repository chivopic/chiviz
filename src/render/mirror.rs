use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

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

        let (bar_width, gap) = if width >= state.values.len() * 3 {
            (2, 1)
        } else if width >= state.values.len() * 2 {
            (1, 1)
        } else {
            (1, 0)
        };

        let unit_stride = bar_width + gap;
        let max_bars = width / unit_stride.max(1);
        let num_bars = state.values.len().min(max_bars);

        let used_width = num_bars * unit_stride;
        let x_offset = if width > used_width {
            (width - used_width) / 2
        } else {
            0
        };

        // Draw center baseline
        for bx in 0..width {
            set_cell(
                buf,
                area,
                area.x as usize + bx,
                area.y as usize + center_y,
                '─',
                Style::default().fg(state.palette.muted),
            );
        }

        // Lower half block characters for downward reflection:
        // '▔', '▀', etc.
        // Or standard blocks: for upper half, grows from center_y up.
        // For lower half, grows from center_y down.
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
                let row_from_center = dy; // 0 is immediately above center line
                let target_y = center_y - 1 - dy;

                let cell_subunit_start = row_from_center * 8;
                let cell_subunit_end = cell_subunit_start + 8;

                let height_ratio = row_from_center as f32 / half_height.max(1) as f32;
                let cell_color = state.palette.interpolate_gradient(height_ratio);
                let cell_style = Style::default().fg(cell_color);

                let ch = if bar_subunits >= cell_subunit_end {
                    '█'
                } else if bar_subunits > cell_subunit_start {
                    let rem = bar_subunits - cell_subunit_start;
                    BLOCK_CHARS[rem.min(8)]
                } else {
                    ' '
                };

                let (final_ch, final_style) = if ch == ' '
                    && peak_subunits >= cell_subunit_start
                    && peak_subunits < cell_subunit_end
                    && peak > 0.02
                {
                    (PEAK_CHAR, Style::default().fg(state.palette.accent))
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

            // 2. Lower half: reflection from center_y + 1 going DOWN to height - 1
            // Reflected bars are slightly attenuated (e.g. 75% height) with darker/muted color
            let refl_subunits = (bar_subunits as f32 * 0.75).round() as usize;
            let remaining_lower = height.saturating_sub(center_y + 1);

            for dy in 0..remaining_lower {
                let row_from_center = dy;
                let target_y = center_y + 1 + dy;

                let cell_subunit_start = row_from_center * 8;
                let cell_subunit_end = cell_subunit_start + 8;

                // Reflection style: dimmer color
                let height_ratio = row_from_center as f32 / half_height.max(1) as f32;
                let orig_color = state.palette.interpolate_gradient(height_ratio);
                let dimmed_color = dim_color(orig_color, 0.45);
                let cell_style = Style::default().fg(dimmed_color);

                // For downward direction, upper blocks:
                // We can use standard blocks or '▀' inverted
                let ch = if refl_subunits >= cell_subunit_end {
                    '█'
                } else if refl_subunits > cell_subunit_start {
                    '▄'
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

fn dim_color(color: ratatui::style::Color, factor: f32) -> ratatui::style::Color {
    match color {
        ratatui::style::Color::Rgb(r, g, b) => {
            let r = (r as f32 * factor).round() as u8;
            let g = (g as f32 * factor).round() as u8;
            let b = (b as f32 * factor).round() as u8;
            ratatui::style::Color::Rgb(r, g, b)
        }
        other => other,
    }
}
