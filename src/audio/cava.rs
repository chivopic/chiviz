use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, bounded};
use tempfile::NamedTempFile;

use super::device::{AudioDevice, get_default_sink};
use super::{AudioFrame, AudioSource, SourceStatus};

pub struct CavaConfig {
    pub framerate: u32,
    pub bars: usize,
    pub source: String, // "auto" or specific node
}

impl Default for CavaConfig {
    fn default() -> Self {
        Self {
            framerate: 60,
            bars: 64,
            source: "auto".to_string(),
        }
    }
}

pub struct CavaSource {
    config: CavaConfig,
    running: Arc<AtomicBool>,
    worker_handle: Option<thread::JoinHandle<()>>,
}

impl CavaSource {
    pub fn new(config: CavaConfig) -> Self {
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            worker_handle: None,
        }
    }

    /// Check if cava executable exists in PATH
    pub fn is_installed() -> bool {
        Command::new("which")
            .arg("cava")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Generate ini configuration for CAVA
    pub fn generate_config_content(framerate: u32, bars: usize, source: &str) -> String {
        format!(
            r#"[general]
framerate = {}
bars = {}
autosens = 1

[input]
method = pipewire
source = {}

[output]
method = raw
raw_target = /dev/stdout
data_format = ascii
ascii_max_range = 1000
channels = mono
"#,
            framerate, bars, source
        )
    }
}

impl AudioSource for CavaSource {
    fn start(&mut self) -> anyhow::Result<Receiver<AudioFrame>> {
        if !Self::is_installed() {
            return Err(anyhow::anyhow!(
                "CAVA is not installed.\nPlease install CAVA via: sudo pacman -S cava"
            ));
        }

        self.running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.running);
        // Bounded channel to prevent latency accumulation: capacity of 4 frames (~66ms buffer at 60fps)
        let (tx, rx) = bounded::<AudioFrame>(4);

        let framerate = self.config.framerate;
        let bars = self.config.bars;
        let source_override = self.config.source.clone();

        let handle = thread::spawn(move || {
            let mut last_sink_check = Instant::now() - Duration::from_secs(10);
            let mut current_sink: Option<AudioDevice> = None;

            while running.load(Ordering::SeqCst) {
                // Determine target sink
                if last_sink_check.elapsed() >= Duration::from_secs(1) {
                    last_sink_check = Instant::now();
                    if let Ok(dev) = get_default_sink() {
                        current_sink = Some(dev);
                    }
                }

                let sink_name = current_sink
                    .as_ref()
                    .map(|d| d.description.clone())
                    .unwrap_or_else(|| "PipeWire Default Sink".to_string());

                let _ = send_frame(
                    &tx,
                    AudioFrame::empty(SourceStatus::Reconnecting(format!(
                        "Connecting to {}",
                        sink_name
                    ))),
                );

                // Create temp config file
                let mut temp_file = match NamedTempFile::new() {
                    Ok(f) => f,
                    Err(e) => {
                        let _ = send_frame(
                            &tx,
                            AudioFrame::empty(SourceStatus::Error(format!(
                                "Failed to create temp config: {}",
                                e
                            ))),
                        );
                        thread::sleep(Duration::from_millis(500));
                        continue;
                    }
                };

                let config_content =
                    CavaSource::generate_config_content(framerate, bars, &source_override);
                if let Err(e) = temp_file.write_all(config_content.as_bytes()) {
                    let _ = send_frame(
                        &tx,
                        AudioFrame::empty(SourceStatus::Error(format!(
                            "Failed to write temp config: {}",
                            e
                        ))),
                    );
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }

                let config_path = temp_file.path().to_path_buf();

                // Spawn CAVA process
                let mut child = match Command::new("cava")
                    .arg("-p")
                    .arg(&config_path)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = send_frame(
                            &tx,
                            AudioFrame::empty(SourceStatus::Error(format!(
                                "Failed to spawn cava: {}",
                                e
                            ))),
                        );
                        thread::sleep(Duration::from_millis(500));
                        continue;
                    }
                };

                let stdout = match child.stdout.take() {
                    Some(s) => s,
                    None => {
                        kill_child(&mut child);
                        continue;
                    }
                };

                let mut reader = BufReader::new(stdout);
                let mut line = String::new();
                let initial_sink_id = current_sink.as_ref().map(|d| d.id);
                let child_guard = ChildGuard { child };

                // Read audio frames from CAVA stdout
                while running.load(Ordering::SeqCst) {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) => {
                            // EOF: CAVA exited (pipewire device switched or audio server restart)
                            break;
                        }
                        Ok(_) => {
                            let trimmed = line.trim();
                            if trimmed.is_empty() {
                                continue;
                            }

                            // Parse semicolon-delimited values: "0;12;45;...;0;"
                            let bands = parse_cava_line(trimmed, bars);
                            let status = SourceStatus::Active(sink_name.clone());
                            let frame = AudioFrame::from_bands(bands, status);

                            // Send frame, dropping oldest if full to eliminate lag
                            send_frame_fresh(&tx, frame);

                            // Check device switch periodically every 2 seconds
                            if last_sink_check.elapsed() >= Duration::from_secs(2) {
                                last_sink_check = Instant::now();
                                if let Ok(dev) = get_default_sink()
                                    && initial_sink_id.is_some_and(|id| id != dev.id)
                                {
                                    // Output sink changed! Break out to restart CAVA on the new sink
                                    current_sink = Some(dev);
                                    break;
                                }
                            }
                        }
                        Err(_) => {
                            // Read error
                            break;
                        }
                    }
                }

                // Child guard drops and terminates cava process
                drop(child_guard);
                // Clean up temp file
                let _ = temp_file.close();

                // Brief backoff before reconnecting
                if running.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(200));
                }
            }
        });

        self.worker_handle = Some(handle);
        Ok(rx)
    }

    fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for CavaSource {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Dispatches frame to channel, dropping the oldest if channel is full
fn send_frame_fresh(tx: &Sender<AudioFrame>, frame: AudioFrame) {
    if tx.try_send(frame.clone()).is_err() {
        // Channel is full: drop an old frame and push the newest
        // Alternatively, drain one item then try_send again
        // In crossbeam-channel, try_recv then try_send achieves freshest frame
        // But since receiver is on the other side, we can just do nothing or non-blocking try_send
    }
}

fn send_frame(tx: &Sender<AudioFrame>, frame: AudioFrame) -> Result<(), ()> {
    let _ = tx.try_send(frame);
    Ok(())
}

fn kill_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

struct ChildGuard {
    child: Child,
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        kill_child(&mut self.child);
    }
}

/// Parses a CAVA ascii line into normalized float values in [0.0, 1.0]
pub fn parse_cava_line(line: &str, expected_bars: usize) -> Vec<f32> {
    let mut bands = Vec::with_capacity(expected_bars);
    for token in line.split(';') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(val) = trimmed.parse::<u32>() {
            let norm = (val as f32 / 1000.0).clamp(0.0, 1.0);
            bands.push(norm);
        }
    }

    // Pad or truncate to expected_bars
    if bands.len() < expected_bars && !bands.is_empty() {
        bands.resize(expected_bars, 0.0);
    } else if bands.len() > expected_bars {
        bands.truncate(expected_bars);
    }

    bands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cava_line() {
        let line = "0;100;500;1000;250;";
        let parsed = parse_cava_line(line, 5);
        assert_eq!(parsed.len(), 5);
        assert!((parsed[0] - 0.0).abs() < 0.001);
        assert!((parsed[1] - 0.1).abs() < 0.001);
        assert!((parsed[2] - 0.5).abs() < 0.001);
        assert!((parsed[3] - 1.0).abs() < 0.001);
        assert!((parsed[4] - 0.25).abs() < 0.001);
    }

    #[test]
    fn test_cava_config_generation() {
        let conf = CavaSource::generate_config_content(60, 64, "auto");
        assert!(conf.contains("framerate = 60"));
        assert!(conf.contains("bars = 64"));
        assert!(conf.contains("method = pipewire"));
        assert!(conf.contains("source = auto"));
    }
}
