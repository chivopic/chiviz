use crate::audio::AudioFrame;

#[derive(Debug, Clone)]
pub struct MotionConfig {
    pub attack_rate: f32,   // Speed of rising values (higher = faster, e.g. 38.0)
    pub release_rate: f32,  // Speed of falling values (e.g. 7.5)
    pub peak_hold_sec: f32, // How long peak hangs before falling (e.g. 0.35s)
    pub peak_gravity: f32,  // Acceleration of falling peak (e.g. 3.5)
    pub frequency_smoothing: bool,
}

impl Default for MotionConfig {
    fn default() -> Self {
        Self {
            attack_rate: 38.0,
            release_rate: 7.5,
            peak_hold_sec: 0.35,
            peak_gravity: 3.5,
            frequency_smoothing: true,
        }
    }
}

pub struct MotionEngine {
    config: MotionConfig,
    bars_count: usize,
    pub values: Vec<f32>,
    pub peaks: Vec<f32>,
    peak_timers: Vec<f32>,
    peak_velocities: Vec<f32>,
    pub sensitivity: f32,
    pub frozen: bool,
    rolling_peak_energy: f32,
    pub smoothed_bass: f32,
    pub smoothed_mid: f32,
    pub smoothed_treble: f32,
    pub smoothed_energy: f32,
}

impl MotionEngine {
    pub fn new(bars_count: usize) -> Self {
        Self {
            config: MotionConfig::default(),
            bars_count,
            values: vec![0.0; bars_count],
            peaks: vec![0.0; bars_count],
            peak_timers: vec![0.0; bars_count],
            peak_velocities: vec![0.0; bars_count],
            sensitivity: 1.0,
            frozen: false,
            rolling_peak_energy: 0.5,
            smoothed_bass: 0.0,
            smoothed_mid: 0.0,
            smoothed_treble: 0.0,
            smoothed_energy: 0.0,
        }
    }

    pub fn resize(&mut self, new_bars: usize) {
        if new_bars != self.bars_count && new_bars > 0 {
            self.bars_count = new_bars;
            self.values.resize(new_bars, 0.0);
            self.peaks.resize(new_bars, 0.0);
            self.peak_timers.resize(new_bars, 0.0);
            self.peak_velocities.resize(new_bars, 0.0);
        }
    }

    pub fn adjust_sensitivity(&mut self, delta: f32) {
        self.sensitivity = (self.sensitivity + delta).clamp(0.2, 5.0);
    }

    pub fn toggle_freeze(&mut self) {
        self.frozen = !self.frozen;
    }

    /// Update animation state with a new incoming audio frame and delta time dt (in seconds).
    pub fn update(&mut self, frame: Option<&AudioFrame>, dt: f32) {
        if self.frozen {
            return;
        }

        let dt = dt.clamp(0.001, 0.1); // Guard against huge frame hiccups

        // Handle case where no frame arrived or source is idle
        let raw_bands = match frame {
            Some(f) if !f.frequency_bands.is_empty() => &f.frequency_bands[..],
            _ => &[],
        };

        // Resample / fit raw_bands to bars_count
        let input_bands = if raw_bands.is_empty() {
            vec![0.0; self.bars_count]
        } else if raw_bands.len() == self.bars_count {
            raw_bands.to_vec()
        } else {
            resample_bands(raw_bands, self.bars_count)
        };

        // Spatial frequency smoothing (3-point kernel)
        let smoothed_input = if self.config.frequency_smoothing && self.bars_count > 3 {
            smooth_spectrum(&input_bands)
        } else {
            input_bands
        };

        // Dynamic Normalization: calculate instantaneous peak in this frame
        let frame_peak = smoothed_input.iter().cloned().fold(0.0f32, f32::max);
        if frame_peak > self.rolling_peak_energy {
            // Fast attack on rolling peak
            self.rolling_peak_energy += (frame_peak - self.rolling_peak_energy) * 0.2;
        } else {
            // Slow decay on rolling peak towards floor of 0.4
            self.rolling_peak_energy = (self.rolling_peak_energy - 0.05 * dt).max(0.4);
        }

        let norm_scale = if self.rolling_peak_energy > 0.05 {
            (1.0 / self.rolling_peak_energy).min(2.5)
        } else {
            1.0
        };

        let attack_coeff = 1.0 - (-self.config.attack_rate * dt).exp();
        let release_coeff = (-self.config.release_rate * dt).exp();

        // Update each bar
        for (i, &band_input) in smoothed_input.iter().enumerate().take(self.bars_count) {
            let target = (band_input * self.sensitivity * norm_scale).clamp(0.0, 1.0);
            let current = self.values[i];

            let new_val = if target >= current {
                // Attack
                current + (target - current) * attack_coeff
            } else {
                // Release
                current * release_coeff
            };
            self.values[i] = new_val.clamp(0.0, 1.0);

            // Peak Hold logic
            if new_val >= self.peaks[i] {
                self.peaks[i] = new_val;
                self.peak_timers[i] = self.config.peak_hold_sec;
                self.peak_velocities[i] = 0.0;
            } else if self.peak_timers[i] > 0.0 {
                self.peak_timers[i] -= dt;
            } else {
                // Peak gravity fall
                self.peak_velocities[i] += self.config.peak_gravity * dt;
                self.peaks[i] = (self.peaks[i] - self.peak_velocities[i] * dt).max(new_val);
            }
        }

        // Smooth energy metrics for orbit / wave modulation
        if let Some(f) = frame {
            let e_attack = 1.0 - (-25.0 * dt).exp();
            let e_release = (-8.0 * dt).exp();

            let update_energy = |curr: f32, target: f32| -> f32 {
                let target = (target * self.sensitivity).clamp(0.0, 1.0);
                if target >= curr {
                    curr + (target - curr) * e_attack
                } else {
                    curr * e_release
                }
            };

            self.smoothed_bass = update_energy(self.smoothed_bass, f.bass_energy);
            self.smoothed_mid = update_energy(self.smoothed_mid, f.mid_energy);
            self.smoothed_treble = update_energy(self.smoothed_treble, f.treble_energy);
            self.smoothed_energy = update_energy(self.smoothed_energy, f.normalized_energy);
        } else {
            self.smoothed_bass *= (-8.0 * dt).exp();
            self.smoothed_mid *= (-8.0 * dt).exp();
            self.smoothed_treble *= (-8.0 * dt).exp();
            self.smoothed_energy *= (-8.0 * dt).exp();
        }
    }
}

/// Linear interpolation to resample input bands array to target length
pub fn resample_bands(src: &[f32], target_len: usize) -> Vec<f32> {
    if src.is_empty() || target_len == 0 {
        return vec![0.0; target_len];
    }
    if src.len() == target_len {
        return src.to_vec();
    }

    let mut out = Vec::with_capacity(target_len);
    let scale = (src.len() - 1) as f32 / (target_len.max(2) - 1) as f32;

    for i in 0..target_len {
        let src_idx = i as f32 * scale;
        let low = src_idx.floor() as usize;
        let high = (low + 1).min(src.len() - 1);
        let frac = src_idx - low as f32;
        let val = src[low] * (1.0 - frac) + src[high] * frac;
        out.push(val);
    }
    out
}

/// 3-point smoothing: 0.2 * left + 0.6 * center + 0.2 * right
fn smooth_spectrum(input: &[f32]) -> Vec<f32> {
    let n = input.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let left = if i > 0 { input[i - 1] } else { input[i] };
        let right = if i + 1 < n { input[i + 1] } else { input[i] };
        let smoothed = left * 0.2 + input[i] * 0.6 + right * 0.2;
        out.push(smoothed);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_motion_attack_and_release() {
        let mut engine = MotionEngine::new(4);
        let mut frame_data = vec![0.8; 4];
        let frame = AudioFrame::from_bands(
            frame_data.clone(),
            crate::audio::SourceStatus::Active("test".into()),
        );

        // First update: should attack quickly upwards
        engine.update(Some(&frame), 0.016);
        assert!(engine.values[0] > 0.3);
        assert!(engine.peaks[0] >= engine.values[0]);

        // After several updates, approaches target
        for _ in 0..10 {
            engine.update(Some(&frame), 0.016);
        }
        assert!(engine.values[0] > 0.7);

        // Silence: values should decay
        frame_data.fill(0.0);
        let silent_frame = AudioFrame::from_bands(
            frame_data,
            crate::audio::SourceStatus::Active("test".into()),
        );
        for _ in 0..20 {
            engine.update(Some(&silent_frame), 0.016);
        }
        assert!(engine.values[0] < 0.2);
    }

    #[test]
    fn test_resample_bands() {
        let src = vec![0.0, 1.0];
        let resampled = resample_bands(&src, 5);
        assert_eq!(resampled.len(), 5);
        assert_eq!(resampled[0], 0.0);
        assert_eq!(resampled[4], 1.0);
        assert!((resampled[2] - 0.5).abs() < 0.01);
    }
}
