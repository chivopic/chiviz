use std::f32::consts::PI;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, bounded};

use super::{AudioFrame, AudioSource, SourceStatus};

pub struct DemoSource {
    bars: usize,
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl DemoSource {
    pub fn new(bars: usize) -> Self {
        Self {
            bars,
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }
}

impl AudioSource for DemoSource {
    fn start(&mut self) -> anyhow::Result<Receiver<AudioFrame>> {
        self.running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.running);
        let (tx, rx) = bounded(4);
        let bars = self.bars;

        let handle = thread::spawn(move || {
            let start_time = Instant::now();
            let frame_duration = Duration::from_micros(1_000_000 / 60); // 60 FPS

            // 124 BPM -> beat every ~0.4838 seconds
            let bpm = 124.0f32;
            let beat_period = 60.0 / bpm;

            while running.load(Ordering::SeqCst) {
                let frame_start = Instant::now();
                let elapsed = start_time.elapsed().as_secs_f32();

                let beat_phase = (elapsed % beat_period) / beat_period;
                let _measure_phase = (elapsed % (beat_period * 4.0)) / (beat_period * 4.0);

                // Kick drum envelope (decays over first 35% of each beat)
                let kick = if beat_phase < 0.35 {
                    (1.0 - (beat_phase / 0.35)).powf(2.0)
                } else {
                    0.0
                };

                // Snare on beat 2 and 4 (measure_phase around 0.25 and 0.75)
                let snare_subbeat = (elapsed + beat_period) % (beat_period * 2.0);
                let snare_phase = (snare_subbeat % beat_period) / beat_period;
                let snare = if snare_phase < 0.25 {
                    (1.0 - (snare_phase / 0.25)).powf(1.8)
                } else {
                    0.0
                };

                // Hi-hat on 16th notes
                let sixteenth_phase = (elapsed % (beat_period / 4.0)) / (beat_period / 4.0);
                let hihat = if sixteenth_phase < 0.15 {
                    1.0 - (sixteenth_phase / 0.15)
                } else {
                    0.0
                };

                // Melody arp sweeping through mid frequencies
                let arp_speed = 3.0; // steps per second
                let arp_note = ((elapsed * arp_speed).sin() * 0.5 + 0.5) * (bars as f32 * 0.5);

                let mut bands = Vec::with_capacity(bars);
                for i in 0..bars {
                    let freq_ratio = i as f32 / bars as f32;
                    let mut val = 0.0f32;

                    // Bass frequencies (lower 20%)
                    if freq_ratio < 0.2 {
                        let bass_weight = 1.0 - (freq_ratio / 0.2);
                        val += kick * 0.95 * bass_weight;
                        // Bassline groove
                        val += ((elapsed * 4.0 + freq_ratio * 10.0).sin() * 0.5 + 0.5) * 0.35;
                    }

                    // Mid frequencies (20% .. 65%)
                    if (0.15..0.7).contains(&freq_ratio) {
                        let dist = (i as f32 - (bars as f32 * 0.2 + arp_note)).abs();
                        if dist < 6.0 {
                            let arp_contrib = (1.0 - (dist / 6.0)).powf(1.5) * 0.7;
                            val += arp_contrib;
                        }
                        val +=
                            snare * 0.5 * (1.0 - ((freq_ratio - 0.4).abs() * 2.0).clamp(0.0, 1.0));
                    }

                    // High frequencies (65% .. 100%)
                    if freq_ratio >= 0.6 {
                        val += hihat * 0.65;
                        val += ((elapsed * 12.0 + i as f32).sin() * 0.5 + 0.5) * 0.2;
                    }

                    // Subtle background energy wave
                    let wave = ((elapsed * 2.0 + freq_ratio * 4.0 * PI).sin() * 0.5 + 0.5) * 0.15;
                    val += wave;

                    bands.push(val.clamp(0.0, 1.0));
                }

                let status = SourceStatus::Active("Demo Mode (Synthetic)".to_string());
                let frame = AudioFrame::from_bands(bands, status);
                let _ = tx.try_send(frame);

                let elapsed_frame = frame_start.elapsed();
                if elapsed_frame < frame_duration {
                    thread::sleep(frame_duration - elapsed_frame);
                }
            }
        });

        self.handle = Some(handle);
        Ok(rx)
    }

    fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for DemoSource {
    fn drop(&mut self) {
        self.stop();
    }
}
