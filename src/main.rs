mod audio;
mod doctor;
mod motion;
mod render;
mod theme;
mod ui;

use std::time::{Duration, Instant};

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};

use crate::audio::cava::{CavaConfig, CavaSource};
use crate::audio::demo::DemoSource;
use crate::audio::device::list_sinks;
use crate::audio::{AudioFrame, AudioSource, SourceStatus};
use crate::motion::MotionEngine;
use crate::render::classic::ClassicRenderer;
use crate::render::mirror::MirrorRenderer;
use crate::render::orbit::OrbitRenderer;
use crate::render::wave::WaveRenderer;
use crate::render::{VisualMode, VisualizerRenderer, VisualizerState};
use crate::theme::{ThemeManager, ThemeMode};
use crate::ui::terminal::TerminalGuard;
use crate::ui::{LayoutPreference, UiState, render_frame};

#[derive(Parser, Debug)]
#[command(
    name = "chiviz",
    author = "chiv",
    version = "0.1.0",
    about = "⚡ Omarchy-Native Audio Visualizer for Kitty and Hyprland",
    long_about = "chiviz is a high-performance, real-time terminal music visualizer crafted specifically for Omarchy, Kitty terminal, and PipeWire."
)]
struct Cli {
    /// Visualization mode: classic, mirror, wave, orbit
    #[arg(short, long)]
    mode: Option<String>,

    /// Visual collection: retro, modern
    #[arg(short, long)]
    collection: Option<String>,

    /// Layout preference: auto, compact, immersive
    #[arg(short, long, default_value = "auto")]
    layout: String,

    /// Theme: auto, accent, mono, tokyonight, catppuccin, neon, winamp
    #[arg(short, long, default_value = "auto")]
    theme: String,

    /// Sensitivity multiplier (0.2 - 5.0)
    #[arg(short, long, default_value_t = 1.0)]
    sensitivity: f32,

    /// Number of frequency bars (16 - 256)
    #[arg(short, long, default_value_t = 64)]
    bars: usize,

    /// Target framerate (FPS)
    #[arg(short, long, default_value_t = 60)]
    framerate: u32,

    /// Audio source ("auto" or specific PipeWire node)
    #[arg(long, default_value = "auto")]
    audio_source: String,

    /// Run with synthetic music demo without PipeWire/CAVA capture
    #[arg(long)]
    demo: bool,

    /// Run system environment and PipeWire diagnostics
    #[arg(long)]
    doctor: bool,

    /// List all available PipeWire audio output devices
    #[arg(long)]
    list_devices: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // 1. Run doctor diagnostics if requested
    if cli.doctor {
        let pass = doctor::run_doctor();
        if pass {
            return Ok(());
        } else {
            std::process::exit(1);
        }
    }

    // 2. List audio devices if requested
    if cli.list_devices {
        println!("\x1b[1;36m=== PipeWire Audio Output Devices ===\x1b[0m\n");
        match list_sinks() {
            Ok(sinks) => {
                for sink in sinks {
                    let star = if sink.is_default {
                        " \x1b[1;32m*\x1b[0m"
                    } else {
                        "  "
                    };
                    println!(
                        "{} \x1b[1m[{}]\x1b[0m {} (vol: {:.0}%{})",
                        star,
                        sink.id,
                        sink.description,
                        sink.volume * 100.0,
                        if sink.is_muted { " [MUTED]" } else { "" }
                    );
                    println!("      node.name: {}", sink.name);
                }
                println!("\n\x1b[33m* Indicates active default audio sink\x1b[0m");
            }
            Err(e) => {
                eprintln!("\x1b[1;31mError querying devices: {}\x1b[0m", e);
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    // 3. Determine initial visual mode
    let mut current_mode = if let Some(m) = &cli.mode {
        VisualMode::from_str_name(m).unwrap_or(VisualMode::Classic)
    } else if let Some(c) = &cli.collection {
        match c.to_lowercase().as_str() {
            "modern" => VisualMode::Wave,
            _ => VisualMode::Classic,
        }
    } else {
        VisualMode::Classic
    };

    // 4. Determine layout preference
    let layout_pref =
        LayoutPreference::from_str_name(&cli.layout).unwrap_or(LayoutPreference::Auto);

    // 5. Determine initial theme
    let theme_mode = ThemeMode::from_str_name(&cli.theme).unwrap_or(ThemeMode::OmarchyAuto);
    let mut theme_mgr = ThemeManager::new(theme_mode);

    // 6. Start audio source
    let bars = cli.bars.clamp(16, 256);
    let framerate = cli.framerate.clamp(30, 120);

    let mut audio_source_box: Box<dyn AudioSource> = if cli.demo {
        Box::new(DemoSource::new(bars))
    } else {
        if !CavaSource::is_installed() {
            eprintln!("\x1b[1;31mError: CAVA is not installed.\x1b[0m");
            eprintln!("chiviz requires CAVA for PipeWire audio spectrum analysis.");
            eprintln!("\nPlease install CAVA with:");
            eprintln!("  \x1b[1;32msudo pacman -S cava\x1b[0m  (or yay -S cava)\n");
            eprintln!("Or run chiviz in demo mode with: \x1b[1mchiviz --demo\x1b[0m");
            std::process::exit(1);
        }
        let conf = CavaConfig {
            framerate,
            bars,
            source: cli.audio_source.clone(),
        };
        Box::new(CavaSource::new(conf))
    };

    let audio_rx = audio_source_box.start()?;

    // 7. Motion engine
    let mut motion_engine = MotionEngine::new(bars);
    motion_engine.sensitivity = cli.sensitivity.clamp(0.2, 5.0);

    // 8. Visualizer renderers
    let mut classic_renderer = ClassicRenderer::new();
    let mut mirror_renderer = MirrorRenderer::new();
    let mut wave_renderer = WaveRenderer::new();
    let mut orbit_renderer = OrbitRenderer::new();

    // 9. UI state & terminal setup
    let mut ui_state = UiState::new(layout_pref);
    let mut terminal = TerminalGuard::init()?;

    let target_frame_time = Duration::from_micros(1_000_000 / framerate as u64);
    let start_time = Instant::now();
    let mut last_tick = Instant::now();
    let mut last_theme_check = Instant::now();

    let mut last_valid_frame: Option<AudioFrame> = None;
    let mut last_frame_received_at = Instant::now();
    let mut current_status = SourceStatus::Reconnecting("Initializing audio stream...".to_string());

    // Main event and rendering loop
    loop {
        let frame_start = Instant::now();
        let dt = last_tick.elapsed().as_secs_f32();
        last_tick = Instant::now();

        // Check for theme updates periodically
        if last_theme_check.elapsed() >= Duration::from_millis(500) {
            last_theme_check = Instant::now();
            theme_mgr.check_for_updates();
        }

        // Poll keyboard events (non-blocking with tiny timeout)
        while event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    // Quit
                    KeyCode::Char('q') => {
                        if ui_state.show_help {
                            ui_state.show_help = false;
                        } else {
                            return Ok(());
                        }
                    }
                    KeyCode::Esc => {
                        if ui_state.show_help {
                            ui_state.show_help = false;
                        } else {
                            return Ok(());
                        }
                    }

                    // Mode switching
                    // Mode switching (Space, 1-4, m, n, p)
                    KeyCode::Char(' ') | KeyCode::Char('m') | KeyCode::Char('n') => {
                        current_mode = current_mode.next();
                        ui_state.show_toast(format!(
                            "{} Mode: {} ({}/4) • {}",
                            current_mode.icon(),
                            current_mode.name(),
                            current_mode.number(),
                            current_mode.collection()
                        ));
                    }
                    KeyCode::Char(c @ '1'..='4') => {
                        if let Some(digit) = c.to_digit(10)
                            && let Some(mode) = VisualMode::from_number(digit as u8)
                        {
                            current_mode = mode;
                            ui_state.show_toast(format!(
                                "{} Mode: {} ({}/4) • {}",
                                current_mode.icon(),
                                current_mode.name(),
                                current_mode.number(),
                                current_mode.collection()
                            ));
                        }
                    }
                    KeyCode::Char('p') => {
                        current_mode = current_mode.prev();
                        ui_state.show_toast(format!(
                            "{} Mode: {} ({}/4) • {}",
                            current_mode.icon(),
                            current_mode.name(),
                            current_mode.number(),
                            current_mode.collection()
                        ));
                    }

                    // Theme switching
                    KeyCode::Tab => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            theme_mgr.current_mode = theme_mgr.current_mode.prev();
                        } else {
                            theme_mgr.current_mode = theme_mgr.current_mode.next();
                        }
                        ui_state.show_toast(format!("🎨 Theme: {}", theme_mgr.current_mode.name()));
                    }
                    KeyCode::BackTab => {
                        theme_mgr.current_mode = theme_mgr.current_mode.prev();
                        ui_state.show_toast(format!("🎨 Theme: {}", theme_mgr.current_mode.name()));
                    }

                    // Layout toggle
                    KeyCode::Char('f') => {
                        let term_size = terminal.terminal_mut().size().unwrap_or_default();
                        let curr_imm = ui_state.is_immersive(term_size.width, term_size.height);
                        ui_state.toggle_immersive(curr_imm);
                        ui_state.show_toast(if !curr_imm {
                            "🖥️ Layout: Immersive"
                        } else {
                            "🪟 Layout: Compact"
                        });
                    }

                    // Sensitivity
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        motion_engine.adjust_sensitivity(0.1);
                        ui_state.show_toast(format!("🎚️ Sensitivity: {:.1}x", motion_engine.sensitivity));
                    }
                    KeyCode::Char('-') | KeyCode::Char('_') => {
                        motion_engine.adjust_sensitivity(-0.1);
                        ui_state.show_toast(format!("🎚️ Sensitivity: {:.1}x", motion_engine.sensitivity));
                    }

                    // Pause / Freeze
                    KeyCode::Char('s') => {
                        motion_engine.toggle_freeze();
                        ui_state.show_toast(if motion_engine.frozen {
                            "⏸️ Animation Paused"
                        } else {
                            "▶️ Animation Resumed"
                        });
                    }

                    // Help
                    KeyCode::Char('?') | KeyCode::Char('h') => {
                        ui_state.show_help = !ui_state.show_help;
                    }

                    _ => {}
                }
            }
        }

        // Drain audio frames from channel to keep up with real-time (discard stale frames)
        let mut newest_frame = None;
        while let Ok(frame) = audio_rx.try_recv() {
            newest_frame = Some(frame);
        }

        if let Some(frame) = newest_frame {
            if frame.timestamp.elapsed() < Duration::from_millis(250) {
                current_status = frame.source_status.clone();
                last_valid_frame = Some(frame);
                last_frame_received_at = Instant::now();
            }
        } else if last_frame_received_at.elapsed() > Duration::from_secs(1) {
            // No audio frames received for over 1 second: silence / idle
            current_status = match current_status {
                SourceStatus::Active(dev) => SourceStatus::Idle(dev),
                other => other,
            };
        }

        // Dynamic bar count adaptation for small windows
        let term_size = terminal.terminal_mut().size().unwrap_or_default();
        let target_bars = if term_size.width < 50 {
            (term_size.width as usize).saturating_sub(4).max(16)
        } else {
            bars
        };
        motion_engine.resize(target_bars);

        // Update motion engine with newest frame
        motion_engine.update(last_valid_frame.as_ref(), dt);

        let active_palette = theme_mgr.current_palette();
        let elapsed_sec = start_time.elapsed().as_secs_f32();

        let visualizer_state = VisualizerState {
            values: &motion_engine.values,
            peaks: &motion_engine.peaks,
            bass_energy: motion_engine.smoothed_bass,
            mid_energy: motion_engine.smoothed_mid,
            treble_energy: motion_engine.smoothed_treble,
            normalized_energy: motion_engine.smoothed_energy,
            palette: &active_palette,
            elapsed_sec,
            is_frozen: motion_engine.frozen,
        };

        // Pick renderer based on current mode
        let active_renderer: &mut dyn VisualizerRenderer = match current_mode {
            VisualMode::Classic => &mut classic_renderer,
            VisualMode::Mirror => &mut mirror_renderer,
            VisualMode::Wave => &mut wave_renderer,
            VisualMode::Orbit => &mut orbit_renderer,
        };

        // Draw to terminal
        terminal.terminal_mut().draw(|frame| {
            let area = frame.area();
            let buf = frame.buffer_mut();
            render_frame(
                buf,
                area,
                active_renderer,
                current_mode,
                &theme_mgr,
                &mut ui_state,
                &current_status,
                &visualizer_state,
                motion_engine.sensitivity,
            );
        })?;

        // Maintain target frame rate (avoid busy loop)
        let elapsed_frame = frame_start.elapsed();
        if elapsed_frame < target_frame_time {
            std::thread::sleep(target_frame_time - elapsed_frame);
        }
    }
}
