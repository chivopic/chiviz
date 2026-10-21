use std::f32::consts::PI;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use super::{VisualizerRenderer, VisualizerState, set_cell};

pub struct WaveRenderer;

impl WaveRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WaveRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl VisualizerRenderer for WaveRenderer {
    fn name(&self) -> &'static str {
        "Smooth Wave"
    }

    fn collection(&self) -> &'static str {
        "Modern"
    }

    fn render(&mut self, buf: &mut Buffer, area: Rect, state: &VisualizerState) {
        if area.width < 4 || area.height < 4 {
            return;
        }

        let width = area.width as usize;
        let height = area.height as usize;

        // Sub-pixel grid: 2 dots horizontally, 4 dots vertically per character
        let sub_width = width * 2;
        let sub_height = height * 4;
        let center_y = sub_height as f32 * 0.55;

        // We use a Braille bitmask buffer: width * height
        let mut braille_grid = vec![0u8; width * height];

        // Synthesize continuous harmonic wave from spectrum
        let time = state.elapsed_sec;
        let bass = state.bass_energy;
        let mid = state.mid_energy;
        let treble = state.treble_energy;
        let energy = state.normalized_energy;

        // Base wave parameters
        let max_amp = (sub_height as f32 * 0.42).max(4.0);

        // Precompute wave height for all sub-pixel columns
        let mut wave_heights = Vec::with_capacity(sub_width);

        let num_bands = state.values.len().max(1);

        for sx in 0..sub_width {
            let norm_x = sx as f32 / sub_width as f32; // 0.0 .. 1.0

            // Sample spectrum value at this x position
            let band_idx = (norm_x * (num_bands - 1) as f32).round() as usize;
            let local_energy = state.values.get(band_idx).copied().unwrap_or(0.0);

            // Multi-harmonic synthesized wave components:
            // 1. Bass carrier wave (long, heavy undulation)
            let w1 = (norm_x * 2.0 * PI * 1.5 - time * 3.5).sin() * (bass * 0.55 + 0.05);

            // 2. Mid wave (musical ripples)
            let w2 = (norm_x * 2.0 * PI * 3.5 + time * 2.0).sin() * (mid * 0.35);

            // 3. Local spectrum envelope modulation
            let w3 = (norm_x * 2.0 * PI * 7.0 - time * 5.0).sin() * (local_energy * 0.25);

            // 4. Treble shimmer (fine high frequency ripples)
            let w4 = (norm_x * 2.0 * PI * 16.0 + time * 8.0).sin() * (treble * 0.15);

            // Combined displacement
            let total_disp = (w1 + w2 + w3 + w4) * max_amp * (energy * 1.2 + 0.15);
            let y = (center_y - total_disp).clamp(1.0, (sub_height - 2) as f32);
            wave_heights.push(y);
        }

        // Connect adjacent points to form a continuous line
        for sx in 0..(sub_width - 1) {
            let y1 = wave_heights[sx];
            let y2 = wave_heights[sx + 1];

            let start_y = y1.min(y2).floor() as usize;
            let end_y = y1.max(y2).ceil() as usize;

            for sy in start_y..=end_y {
                if sy < sub_height {
                    set_braille_dot(&mut braille_grid, width, sx, sy);
                }
            }
        }

        // Draw Braille characters to Ratatui buffer
        for cy in 0..height {
            let y_ratio = cy as f32 / height as f32;
            let color = state.palette.interpolate_gradient(y_ratio);
            let style = Style::default().fg(color);

            for cx in 0..width {
                let mask = braille_grid[cy * width + cx];
                if mask != 0 {
                    let ch = char::from_u32(0x2800 + mask as u32).unwrap_or(' ');
                    set_cell(
                        buf,
                        area,
                        area.x as usize + cx,
                        area.y as usize + cy,
                        ch,
                        style,
                    );
                }
            }
        }

        // Subtitle note: explicitly note that this is synthesized from frequency spectrum
        let note = " [Harmonic Wave • Synthetic Spectrum] ";
        if area.height >= 6 && area.width >= (note.len() as u16 + 4) {
            let start_x = area.x + 2;
            let start_y = area.bottom() - 1;
            let note_style = Style::default().fg(state.palette.muted);

            for (i, ch) in note.chars().enumerate() {
                set_cell(
                    buf,
                    area,
                    (start_x + i as u16) as usize,
                    start_y as usize,
                    ch,
                    note_style,
                );
            }
        }
    }
}

/// Sets a sub-pixel dot in the Braille grid
#[inline]
fn set_braille_dot(grid: &mut [u8], width: usize, sx: usize, sy: usize) {
    let cx = sx / 2;
    let cy = sy / 4;
    let dot_x = sx % 2;
    let dot_y = sy % 4;

    let dot_mask = match (dot_x, dot_y) {
        (0, 0) => 0x01,
        (0, 1) => 0x02,
        (0, 2) => 0x04,
        (0, 3) => 0x40,
        (1, 0) => 0x08,
        (1, 1) => 0x10,
        (1, 2) => 0x20,
        (1, 3) => 0x80,
        _ => 0,
    };

    let idx = cy * width + cx;
    if idx < grid.len() {
        grid[idx] |= dot_mask;
    }
}
