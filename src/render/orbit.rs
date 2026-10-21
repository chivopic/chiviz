use std::f32::consts::PI;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use super::{VisualizerRenderer, VisualizerState, set_cell};

pub struct OrbitRenderer;

impl OrbitRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OrbitRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl VisualizerRenderer for OrbitRenderer {
    fn name(&self) -> &'static str {
        "Orbit"
    }

    fn collection(&self) -> &'static str {
        "Modern"
    }

    fn render(&mut self, buf: &mut Buffer, area: Rect, state: &VisualizerState) {
        if area.width < 10 || area.height < 6 {
            return;
        }

        let width = area.width as usize;
        let height = area.height as usize;

        // Sub-pixel Braille canvas
        let sub_width = width * 2;
        let sub_height = height * 4;

        let center_x = sub_width as f32 * 0.5;
        let center_y = sub_height as f32 * 0.5;

        let mut braille_grid = vec![0u8; width * height];

        // Terminal font aspect ratio correction for Braille:
        // Typical Kitty font: width/height ~ 0.5.
        // Braille has 2 cols, 4 rows. So each sub-pixel is (1/2):(1/4)*2 = 1:1.
        // We add a fine-tuning factor of 1.05 to maintain perfect circularity.
        let aspect_ratio = 1.05f32;

        let max_dim = (sub_width as f32).min(sub_height as f32 * aspect_ratio);
        let base_radius = max_dim * 0.18;
        let max_spike_len = max_dim * 0.28;

        // Bass energy breathes/expands the inner radius
        let bass = state.bass_energy;
        let treble = state.treble_energy;
        let time = state.elapsed_sec;

        let inner_radius = base_radius * (1.0 + bass * 0.5);

        let num_bands = state.values.len();
        let num_rays = num_bands.clamp(32, 128);

        // 1. Draw inner pulsating circle
        let circle_steps = 180;
        for step in 0..circle_steps {
            let theta = (step as f32 / circle_steps as f32) * 2.0 * PI;
            let rx = inner_radius * theta.cos();
            let ry = (inner_radius * theta.sin()) / aspect_ratio;

            let sx = (center_x + rx).round() as isize;
            let sy = (center_y + ry).round() as isize;

            if sx >= 0 && sx < sub_width as isize && sy >= 0 && sy < sub_height as isize {
                set_braille_dot(&mut braille_grid, width, sx as usize, sy as usize);
            }
        }

        // 2. Trace outer spectrum perimeter & radial spikes
        let angular_steps = 360;
        let mut prev_outer_x = None;
        let mut prev_outer_y = None;

        for step in 0..=angular_steps {
            let theta = (step as f32 / angular_steps as f32) * 2.0 * PI;
            // Map angle to spectrum bin (0 at top, clockwise)
            let norm_angle = (theta / (2.0 * PI)).clamp(0.0, 1.0);

            // Symmetrical wrap: 0 to 0.5 is left-to-right, 0.5 to 1.0 is mirrored
            let band_ratio = if norm_angle <= 0.5 {
                norm_angle * 2.0
            } else {
                (1.0 - norm_angle) * 2.0
            };

            let band_idx = ((band_ratio * (num_bands - 1) as f32).round() as usize)
                .min(num_bands.saturating_sub(1));
            let val = state.values.get(band_idx).copied().unwrap_or(0.0);

            // Treble shimmer
            let shimmer = (theta * 24.0 + time * 6.0).sin() * treble * 2.5;

            let current_radius = inner_radius + val * max_spike_len + shimmer;

            let rx = current_radius * theta.cos();
            let ry = (current_radius * theta.sin()) / aspect_ratio;

            let outer_x = (center_x + rx).round() as isize;
            let outer_y = (center_y + ry).round() as isize;

            // Connect outer perimeter
            if let (Some(px), Some(py)) = (prev_outer_x, prev_outer_y) {
                draw_braille_line(
                    &mut braille_grid,
                    width,
                    sub_width,
                    sub_height,
                    px,
                    py,
                    outer_x,
                    outer_y,
                );
            }
            prev_outer_x = Some(outer_x);
            prev_outer_y = Some(outer_y);

            // Periodically draw radial spokes from inner radius to outer spike
            if step % (angular_steps / num_rays).max(1) == 0 && val > 0.08 {
                let inner_rx = inner_radius * theta.cos();
                let inner_ry = (inner_radius * theta.sin()) / aspect_ratio;
                let in_x = (center_x + inner_rx).round() as isize;
                let in_y = (center_y + inner_ry).round() as isize;
                draw_braille_line(
                    &mut braille_grid,
                    width,
                    sub_width,
                    sub_height,
                    in_x,
                    in_y,
                    outer_x,
                    outer_y,
                );
            }
        }

        // Render Braille grid to buffer with radial color gradient
        for cy in 0..height {
            let cell_y = area.y as usize + cy;
            if cell_y >= area.bottom() as usize {
                continue;
            }

            for cx in 0..width {
                let mask = braille_grid[cy * width + cx];
                if mask != 0 {
                    let cell_x = area.x as usize + cx;
                    if cell_x >= area.right() as usize {
                        continue;
                    }

                    // Compute distance from center for radial gradient coloring
                    let sub_x = (cx * 2 + 1) as f32;
                    let sub_y = (cy * 4 + 2) as f32;
                    let dx = sub_x - center_x;
                    let dy = (sub_y - center_y) * aspect_ratio;
                    let dist = (dx * dx + dy * dy).sqrt();

                    let grad_ratio = ((dist - inner_radius * 0.7) / (max_spike_len * 1.2).max(1.0))
                        .clamp(0.0, 1.0);
                    let color = state.palette.interpolate_gradient(grad_ratio);
                    let style = Style::default().fg(color);

                    let ch = char::from_u32(0x2800 + mask as u32).unwrap_or(' ');
                    set_cell(buf, area, cell_x, cell_y, ch, style);
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

/// Bresenham's line algorithm on sub-pixel Braille canvas
#[allow(clippy::too_many_arguments)]
fn draw_braille_line(
    grid: &mut [u8],
    width: usize,
    sub_width: usize,
    sub_height: usize,
    mut x0: isize,
    mut y0: isize,
    x1: isize,
    y1: isize,
) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        if x0 >= 0 && x0 < sub_width as isize && y0 >= 0 && y0 < sub_height as isize {
            set_braille_dot(grid, width, x0 as usize, y0 as usize);
        }

        if x0 == x1 && y0 == y1 {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}
