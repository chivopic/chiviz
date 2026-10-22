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
        let center_y = sub_height as f32 * 0.52;

        let mut braille_grid = vec![0u8; width * height];
        let mut echo_grid = vec![0u8; width * height];

        let time = state.elapsed_sec;
        let bass = state.bass_energy;
        let mid = state.mid_energy;
        let treble = state.treble_energy;
        let energy = state.normalized_energy;

        let max_amp = (sub_height as f32 * 0.44).max(4.0);
        let num_bands = state.values.len().max(1);

        let mut primary_heights = Vec::with_capacity(sub_width);
        let mut echo_heights = Vec::with_capacity(sub_width);

        // Compute primary wave & harmonic echo wave
        for sx in 0..sub_width {
            let norm_x = sx as f32 / sub_width as f32; // 0.0 .. 1.0

            // Sample local spectrum energy
            let band_idx = (norm_x * (num_bands - 1) as f32).round() as usize;
            let local_val = state.values.get(band_idx).copied().unwrap_or(0.0);

            // 1. Primary multi-harmonic wave components:
            // Bass carrier (undulating wave)
            let w1 = (norm_x * 2.0 * PI * 1.5 - time * 3.8).sin() * (bass * 0.60 + 0.06);
            // Mid harmonic (fluid ripples)
            let w2 = (norm_x * 2.0 * PI * 3.5 + time * 2.2).sin() * (mid * 0.38);
            // Local spectrum peak modulation
            let w3 = (norm_x * 2.0 * PI * 7.5 - time * 5.5).sin() * (local_val * 0.30);
            // Treble shimmer
            let w4 = (norm_x * 2.0 * PI * 16.0 + time * 9.0).sin() * (treble * 0.18);

            let primary_disp = (w1 + w2 + w3 + w4) * max_amp * (energy * 1.25 + 0.18);
            let y1 = (center_y - primary_disp).clamp(1.0, (sub_height - 2) as f32);
            primary_heights.push(y1);

            // 2. Secondary Harmonic Echo Wave (Phase offset, lower amplitude)
            let echo_w1 = (norm_x * 2.0 * PI * 1.5 - time * 3.8 + 0.8).sin() * (bass * 0.40);
            let echo_w2 = (norm_x * 2.0 * PI * 3.5 + time * 2.2 - 0.6).sin() * (mid * 0.25);
            let echo_disp = (echo_w1 + echo_w2) * max_amp * (energy * 0.95 + 0.10);
            let y2 = (center_y - echo_disp).clamp(1.0, (sub_height - 2) as f32);
            echo_heights.push(y2);
        }

        // Connect adjacent points to form continuous curves
        for sx in 0..(sub_width - 1) {
            // Primary wave
            let py1 = primary_heights[sx];
            let py2 = primary_heights[sx + 1];
            let p_start = py1.min(py2).floor() as usize;
            let p_end = py1.max(py2).ceil() as usize;
            for sy in p_start..=p_end {
                if sy < sub_height {
                    set_braille_dot(&mut braille_grid, width, sx, sy);
                }
            }

            // Echo wave
            let ey1 = echo_heights[sx];
            let ey2 = echo_heights[sx + 1];
            let e_start = ey1.min(ey2).floor() as usize;
            let e_end = ey1.max(ey2).ceil() as usize;
            for sy in e_start..=e_end {
                if sy < sub_height {
                    set_braille_dot(&mut echo_grid, width, sx, sy);
                }
            }

            // High-energy ribbon particle fill between wave and centerline
            if energy > 0.18 && sx % 2 == 0 {
                let fill_start = py1.min(center_y).round() as usize;
                let fill_end = py1.max(center_y).round() as usize;
                let step = if energy > 0.5 { 2 } else { 3 };
                for sy in (fill_start..fill_end).step_by(step) {
                    if sy < sub_height && (sx + sy) % 3 == 0 {
                        set_braille_dot(&mut braille_grid, width, sx, sy);
                    }
                }
            }
        }

        // Draw Braille characters to Ratatui buffer
        for cy in 0..height {
            let y_ratio = cy as f32 / height as f32;
            let primary_color = state.palette.interpolate_gradient(y_ratio);
            let primary_style = Style::default().fg(primary_color);

            let echo_color = dim_color(primary_color, 0.45);
            let echo_style = Style::default().fg(echo_color);

            for cx in 0..width {
                let p_mask = braille_grid[cy * width + cx];
                let e_mask = echo_grid[cy * width + cx];

                if p_mask != 0 {
                    let ch = char::from_u32(0x2800 + p_mask as u32).unwrap_or(' ');
                    set_cell(
                        buf,
                        area,
                        area.x as usize + cx,
                        area.y as usize + cy,
                        ch,
                        primary_style,
                    );
                } else if e_mask != 0 {
                    let ch = char::from_u32(0x2800 + e_mask as u32).unwrap_or(' ');
                    set_cell(
                        buf,
                        area,
                        area.x as usize + cx,
                        area.y as usize + cy,
                        ch,
                        echo_style,
                    );
                }
            }
        }
    }
}

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
