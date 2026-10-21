use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use tempfile::NamedTempFile;

use crate::audio::cava::{CavaSource, parse_cava_line};
use crate::audio::device::{get_default_sink, list_sinks};
use crate::render::KittyGraphicsProbe;
use crate::theme::ThemeManager;

pub struct DoctorCheck {
    pub category: &'static str,
    pub title: &'static str,
    pub status: CheckStatus,
    pub details: Vec<String>,
    pub suggestion: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

pub fn run_doctor() -> bool {
    println!("\x1b[1;36m=== chiviz Environment Doctor ===\x1b[0m\n");

    let checks = vec![
        check_terminal(),
        check_pipewire(),
        check_audio_devices(),
        check_cava_binary(),
        check_cava_real_capture(),
        check_omarchy_theme(),
    ];

    let mut all_pass = true;
    for check in &checks {
        let status_tag = match check.status {
            CheckStatus::Pass => "\x1b[1;32m[PASS]\x1b[0m",
            CheckStatus::Warn => "\x1b[1;33m[WARN]\x1b[0m",
            CheckStatus::Fail => {
                all_pass = false;
                "\x1b[1;31m[FAIL]\x1b[0m"
            }
        };

        println!(
            "{} \x1b[1m{}\x1b[0m: {}",
            status_tag, check.category, check.title
        );
        for d in &check.details {
            println!("       • {}", d);
        }
        if let Some(s) = &check.suggestion {
            println!("       \x1b[33m➜ Fix: {}\x1b[0m", s);
        }
        println!();
    }

    if all_pass {
        println!(
            "\x1b[1;32m✨ All system checks passed! chiviz is fully ready for Omarchy.\x1b[0m\n"
        );
    } else {
        println!("\x1b[1;33m⚠ Some checks failed or warned. Review suggestions above.\x1b[0m\n");
    }

    all_pass
}

fn check_terminal() -> DoctorCheck {
    let probe = KittyGraphicsProbe::detect();
    let term = std::env::var("TERM").unwrap_or_default();
    let colorterm = std::env::var("COLORTERM").unwrap_or_default();

    let kitty_ver = Command::new("kitty")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "Not detected".to_string());

    let mut details = vec![
        format!("TERM = {}", if term.is_empty() { "(empty)" } else { &term }),
        format!(
            "COLORTERM = {}",
            if colorterm.is_empty() {
                "(empty)"
            } else {
                &colorterm
            }
        ),
        format!("Kitty binary: {}", kitty_ver),
        format!("TrueColor Support: {}", probe.supports_truecolor),
    ];

    if probe.in_tmux {
        details.push("Running inside tmux session (tmux protocol forwarding)".to_string());
    }

    if probe.is_kitty {
        DoctorCheck {
            category: "Terminal",
            title: "Kitty & TrueColor capability",
            status: CheckStatus::Pass,
            details,
            suggestion: None,
        }
    } else {
        DoctorCheck {
            category: "Terminal",
            title: "Non-Kitty terminal or Kitty variables unset",
            status: CheckStatus::Warn,
            details,
            suggestion: Some(
                "chiviz works best in Kitty. Launch inside Kitty for optimal experience."
                    .to_string(),
            ),
        }
    }
}

fn check_pipewire() -> DoctorCheck {
    let pw_status = Command::new("wpctl").arg("status").output();

    match pw_status {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let first_line = text.lines().next().unwrap_or("PipeWire active").to_string();
            DoctorCheck {
                category: "Audio Server",
                title: "PipeWire & WirePlumber running",
                status: CheckStatus::Pass,
                details: vec![
                    first_line,
                    "wpctl command accessible and communicating with daemon".to_string(),
                ],
                suggestion: None,
            }
        }
        _ => DoctorCheck {
            category: "Audio Server",
            title: "PipeWire / WirePlumber not detected",
            status: CheckStatus::Fail,
            details: vec!["Failed to run 'wpctl status'".to_string()],
            suggestion: Some("Ensure pipewire and wireplumber services are running via systemctl --user start pipewire wireplumber".to_string()),
        },
    }
}

fn check_audio_devices() -> DoctorCheck {
    match get_default_sink() {
        Ok(dev) => {
            let sinks = list_sinks().unwrap_or_default();
            let mut details = vec![
                format!("Active Default Sink: {} (ID: {})", dev.description, dev.id),
                format!("Node Name: {}", dev.name),
                format!(
                    "Volume: {:.0}%{}",
                    dev.volume * 100.0,
                    if dev.is_muted { " [MUTED]" } else { "" }
                ),
                format!("Total Available Sinks: {}", sinks.len()),
            ];
            for s in sinks {
                details.push(format!("  - [{}] {}", s.id, s.display_label()));
            }

            DoctorCheck {
                category: "Output Device",
                title: "Default audio output discovered",
                status: CheckStatus::Pass,
                details,
                suggestion: None,
            }
        }
        Err(e) => DoctorCheck {
            category: "Output Device",
            title: "No default audio sink found",
            status: CheckStatus::Fail,
            details: vec![format!("Error: {}", e)],
            suggestion: Some("Check audio output device in system settings or wpctl".to_string()),
        },
    }
}

fn check_cava_binary() -> DoctorCheck {
    let which_cava = Command::new("which").arg("cava").output();
    let version = Command::new("cava").arg("-v").output();

    match (which_cava, version) {
        (Ok(w), Ok(v)) if w.status.success() && v.status.success() => {
            let path = String::from_utf8_lossy(&w.stdout).trim().to_string();
            let ver = String::from_utf8_lossy(&v.stdout).trim().to_string();
            DoctorCheck {
                category: "CAVA Engine",
                title: "CAVA installed and verified",
                status: CheckStatus::Pass,
                details: vec![format!("Path: {}", path), format!("Version: {}", ver)],
                suggestion: None,
            }
        }
        _ => DoctorCheck {
            category: "CAVA Engine",
            title: "CAVA binary missing",
            status: CheckStatus::Fail,
            details: vec!["'cava' executable was not found in PATH".to_string()],
            suggestion: Some(
                "Install CAVA with: sudo pacman -S cava (or paru -S cava)".to_string(),
            ),
        },
    }
}

fn check_cava_real_capture() -> DoctorCheck {
    let mut temp = match NamedTempFile::new() {
        Ok(t) => t,
        Err(e) => {
            return DoctorCheck {
                category: "Live Capture",
                title: "Failed to create temp config",
                status: CheckStatus::Fail,
                details: vec![e.to_string()],
                suggestion: None,
            };
        }
    };

    let conf = CavaSource::generate_config_content(60, 64, "auto");
    if let Err(e) = temp.write_all(conf.as_bytes()) {
        return DoctorCheck {
            category: "Live Capture",
            title: "Failed to write temp config",
            status: CheckStatus::Fail,
            details: vec![e.to_string()],
            suggestion: None,
        };
    }

    let start = Instant::now();
    let child = Command::new("cava")
        .arg("-p")
        .arg(temp.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();

    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            return DoctorCheck {
                category: "Live Capture",
                title: "Failed to spawn cava test",
                status: CheckStatus::Fail,
                details: vec![e.to_string()],
                suggestion: Some("Verify CAVA PipeWire support with: cava -p <config>".to_string()),
            };
        }
    };

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            let _ = child.kill();
            return DoctorCheck {
                category: "Live Capture",
                title: "Could not capture cava stdout",
                status: CheckStatus::Fail,
                details: vec![],
                suggestion: None,
            };
        }
    };

    let mut reader = BufReader::new(stdout);
    let mut frames_received = 0;
    let mut non_zero_frames = 0;
    let mut line = String::new();

    while start.elapsed() < Duration::from_millis(1500) && frames_received < 15 {
        line.clear();
        if let Ok(n) = reader.read_line(&mut line) {
            if n == 0 {
                break;
            }
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                frames_received += 1;
                let bands = parse_cava_line(trimmed, 64);
                let energy: f32 = bands.iter().sum();
                if energy > 0.001 {
                    non_zero_frames += 1;
                }
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    if frames_received > 0 {
        let details = vec![
            format!(
                "Received {} test frames from CAVA raw PipeWire output",
                frames_received
            ),
            format!(
                "Frames with active audio energy: {}/{}",
                non_zero_frames, frames_received
            ),
            if non_zero_frames > 0 {
                "Active audio detected! Real spectrum pipeline verified.".to_string()
            } else {
                "Pipeline functioning, audio is currently silent or paused.".to_string()
            },
        ];

        DoctorCheck {
            category: "Live Capture",
            title: "PipeWire -> CAVA -> ASCII stream verified",
            status: CheckStatus::Pass,
            details,
            suggestion: None,
        }
    } else {
        DoctorCheck {
            category: "Live Capture",
            title: "No frames received from CAVA",
            status: CheckStatus::Fail,
            details: vec!["CAVA did not produce ascii output within 1.5 seconds".to_string()],
            suggestion: Some("Ensure PipeWire is running and audio is not blocked".to_string()),
        }
    }
}

fn check_omarchy_theme() -> DoctorCheck {
    let mgr = ThemeManager::new(crate::theme::ThemeMode::OmarchyAuto);

    if let Some(pal) = &mgr.omarchy_palette {
        DoctorCheck {
            category: "Omarchy Theme",
            title: "Omarchy colors.toml detected & parsed",
            status: CheckStatus::Pass,
            details: vec![
                format!("Theme Name: {}", mgr.omarchy_theme_name),
                format!("Background: {:?}", pal.background),
                format!("Foreground: {:?}", pal.foreground),
                format!("Accent: {:?}", pal.accent),
                format!("Muted: {:?}", pal.muted),
                format!("Gradient stops: {} colors", pal.gradient.len()),
            ],
            suggestion: None,
        }
    } else {
        DoctorCheck {
            category: "Omarchy Theme",
            title: "Omarchy colors.toml not found, falling back to built-in presets",
            status: CheckStatus::Warn,
            details: vec![
                "Path ~/.local/state/omarchy/current/theme/colors.toml could not be read"
                    .to_string(),
                "Built-in presets (Tokyo Night, Catppuccin, Neon, Winamp) remain fully available"
                    .to_string(),
            ],
            suggestion: Some(
                "If running outside Omarchy, use preset themes with --theme tokyonight".to_string(),
            ),
        }
    }
}
