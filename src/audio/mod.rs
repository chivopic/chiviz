pub mod cava;
pub mod demo;
pub mod device;

use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceStatus {
    /// Actively receiving audio frames from the specified device
    Active(String),
    /// Audio source connected, but currently silent / paused
    Idle(String),
    /// Reconnecting or switching device
    Reconnecting(String),
    /// Source failed with an error
    Error(String),
}

impl SourceStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, SourceStatus::Active(_))
    }

    pub fn label(&self) -> &str {
        match self {
            SourceStatus::Active(s) => s,
            SourceStatus::Idle(s) => s,
            SourceStatus::Reconnecting(s) => s,
            SourceStatus::Error(s) => s,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioFrame {
    pub timestamp: Instant,
    pub frequency_bands: Vec<f32>,
    pub normalized_energy: f32,
    pub bass_energy: f32,
    pub mid_energy: f32,
    pub treble_energy: f32,
    pub source_status: SourceStatus,
}

impl AudioFrame {
    /// Constructs an AudioFrame from raw normalized frequency bands [0.0, 1.0].
    /// Computes aggregated bass, mid, treble, and total normalized energy.
    pub fn from_bands(bands: Vec<f32>, status: SourceStatus) -> Self {
        let n = bands.len();
        if n == 0 {
            return Self::empty(status);
        }

        // Bass: lowest ~18% of bands
        // Mid: ~18% .. 65% of bands
        // Treble: upper ~35% of bands
        let bass_cutoff = (n as f32 * 0.18).round().max(1.0) as usize;
        let mid_cutoff = (n as f32 * 0.65).round().max((bass_cutoff + 1) as f32) as usize;

        let bass_slice = &bands[..bass_cutoff.min(n)];
        let mid_slice = if bass_cutoff < n {
            &bands[bass_cutoff..mid_cutoff.min(n)]
        } else {
            &[]
        };
        let treble_slice = if mid_cutoff < n {
            &bands[mid_cutoff..]
        } else {
            &[]
        };

        let bass_energy = compute_mean_rms(bass_slice);
        let mid_energy = compute_mean_rms(mid_slice);
        let treble_energy = compute_mean_rms(treble_slice);
        let normalized_energy = compute_mean_rms(&bands);

        // Check for silence/idle: if overall energy is basically zero
        let effective_status = if normalized_energy < 0.002 {
            match status {
                SourceStatus::Active(dev) => SourceStatus::Idle(dev),
                other => other,
            }
        } else {
            status
        };

        Self {
            timestamp: Instant::now(),
            frequency_bands: bands,
            normalized_energy,
            bass_energy,
            mid_energy,
            treble_energy,
            source_status: effective_status,
        }
    }

    pub fn empty(status: SourceStatus) -> Self {
        Self {
            timestamp: Instant::now(),
            frequency_bands: Vec::new(),
            normalized_energy: 0.0,
            bass_energy: 0.0,
            mid_energy: 0.0,
            treble_energy: 0.0,
            source_status: status,
        }
    }
}

fn compute_mean_rms(slice: &[f32]) -> f32 {
    if slice.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = slice.iter().map(|&x| x * x).sum();
    (sum_sq / slice.len() as f32).sqrt().clamp(0.0, 1.0)
}

pub trait AudioSource: Send + 'static {
    fn start(&mut self) -> anyhow::Result<crossbeam_channel::Receiver<AudioFrame>>;
    fn stop(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_frame_energy_aggregation() {
        let mut bands = vec![0.0; 64];
        // Low frequency pulse
        for b in bands.iter_mut().take(8) {
            *b = 0.8;
        }

        let frame = AudioFrame::from_bands(bands, SourceStatus::Active("Test".to_string()));
        assert!(frame.bass_energy > 0.4);
        assert!(frame.treble_energy < 0.01);
        assert!(frame.normalized_energy > 0.1);
        assert!(frame.source_status.is_active());
    }

    #[test]
    fn test_silence_idle_detection() {
        let bands = vec![0.0; 64];
        let frame = AudioFrame::from_bands(bands, SourceStatus::Active("Test".to_string()));
        assert_eq!(frame.source_status, SourceStatus::Idle("Test".to_string()));
    }
}
