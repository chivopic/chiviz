use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

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

        // Determine bar layout:
        // Try spacing: 1 char gap if enough room, else 0 gap
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

        // Center horizontally
        let used_width = num_bars * unit_stride;
        let x_offset = if width > used_width {
            (width - used_width) / 2
        } else {
            0
        };

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
                let cell_style = Style::default().fg(cell_color);

                // Determine character in this cell
                let ch = if bar_subunits >= cell_subunit_end {
                    // Fully covered cell
                    '█'
                } else if bar_subunits > cell_subunit_start {
                    // Partially covered cell
                    let rem = bar_subunits - cell_subunit_start;
                    BLOCK_CHARS[rem.min(8)]
                } else {
                    ' '
                };

                // Check peak character
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
                            area.y as usize + dy,
                            final_ch,
                            final_style,
                        );
                    }
                }
            }
        }
    }
}
