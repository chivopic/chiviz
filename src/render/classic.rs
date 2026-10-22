use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::{BLOCK_CHARS, PEAK_CHAR, VisualizerRenderer, VisualizerState, set_cell};

pub struct ClassicRenderer;

impl ClassicRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClassicRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl VisualizerRenderer for ClassicRenderer {
    fn name(&self) -> &'static str {
        "Classic Bars"
    }

    fn collection(&self) -> &'static str {
        "Retro"
    }

    fn render(&mut self, buf: &mut Buffer, area: Rect, state: &VisualizerState) {
        if area.width < 2 || area.height < 2 || state.values.is_empty() {
            return;
        }

        let width = area.width as usize;
        let height = area.height as usize;
        let total_subunits = height * 8;

        // Dynamic bar width calculation: adapt smoothly to terminal width
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

        // Center horizontally
        let used_width = num_bars * unit_stride;
        let x_offset = if width > used_width {
            (width - used_width) / 2
        } else {
            0
        };

        // Bass baseline pulse glow
        let bass = state.bass_energy;
        let bottom_y = area.y as usize + height.saturating_sub(1);

        for bar_idx in 0..num_bars {
            let val = state.values[bar_idx].clamp(0.0, 1.0);
            let peak = state
                .peaks
                .get(bar_idx)
                .copied()
                .unwrap_or(val)
                .clamp(0.0, 1.0);

            let bar_subunits = ((val * total_subunits as f32).round() as usize).min(total_subunits);
            let peak_subunits =
                ((peak * total_subunits as f32).round() as usize).min(total_subunits);

            let start_x = area.x as usize + x_offset + bar_idx * unit_stride;

            for dy in 0..height {
                // dy = 0 is top of the area, dy = height - 1 is bottom
                let row_from_bottom = height - 1 - dy;
                let cell_subunit_start = row_from_bottom * 8;
                let cell_subunit_end = cell_subunit_start + 8;

                // Color along the vertical gradient
                let height_ratio = row_from_bottom as f32 / height.max(1) as f32;
                let cell_color = state.palette.interpolate_gradient(height_ratio);
                let mut cell_style = Style::default().fg(cell_color);

                // Determine character in this cell
                let (ch, is_tip) = if bar_subunits >= cell_subunit_end {
                    ('█', false)
                } else if bar_subunits > cell_subunit_start {
                    let rem = bar_subunits - cell_subunit_start;
                    (BLOCK_CHARS[rem.min(8)], true)
                } else {
                    (' ', false)
                };

                // Highlight the bar top tip for a neon glowing crest
                if is_tip {
                    cell_style = cell_style.add_modifier(Modifier::BOLD);
                }

                // Check peak character
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
                            area.y as usize + dy,
                            final_ch,
                            final_style,
                        );
                    }
                }
            }

            // Draw bass reactive baseline indicator at the very bottom if bar is silent
            if bar_subunits == 0 && bass > 0.25 {
                let baseline_color = if bass > 0.55 {
                    state.palette.accent
                } else {
                    state.palette.muted
                };
                for bx in 0..bar_width {
                    set_cell(
                        buf,
                        area,
                        start_x + bx,
                        bottom_y,
                        ' ',
                        Style::default().fg(baseline_color),
                    );
                }
            }
        }
    }
}
