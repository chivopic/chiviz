use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub struct AudioDevice {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub is_default: bool,
    pub volume: f32,
    pub is_muted: bool,
}

impl AudioDevice {
    pub fn display_label(&self) -> String {
        let default_mark = if self.is_default { " *" } else { "" };
        let mute_mark = if self.is_muted { " [MUTED]" } else { "" };
        format!(
            "{}{}: {} (vol: {:.0}%{})",
            self.id,
            default_mark,
            self.description,
            self.volume * 100.0,
            mute_mark
        )
    }
}

/// Parses the output of `wpctl status` to extract audio sinks.
pub fn parse_wpctl_sinks(output: &str) -> Vec<AudioDevice> {
    let mut sinks = Vec::new();
    let mut in_sinks_section = false;

    for line in output.lines() {
        let trimmed = line.trim();

        if trimmed.contains("Sinks:") {
            in_sinks_section = true;
            continue;
        }

        // Sinks section ends when next section starts (Sources, Filters, Streams, etc.)
        if in_sinks_section {
            if trimmed.contains("Sources:")
                || trimmed.contains("Filters:")
                || trimmed.contains("Streams:")
                || trimmed.contains("Video")
                || trimmed.contains("Settings")
            {
                break;
            }

            // A sink line looks like:
            // " │  *   81. Chiv的AirPods Pro                  [vol: 0.20]"
            // or " │      33. Built-in Audio Analog Stereo        [vol: 0.00]"
            // Clean tree markers
            let clean = line.replace(['│', '├', '└', '─'], " ");
            let clean = clean.trim();

            if clean.is_empty() {
                continue;
            }

            let is_default = clean.starts_with('*');
            let rest = if is_default {
                clean.strip_prefix('*').unwrap_or(clean).trim()
            } else {
                clean
            };

            // Parse ID: starts with digits followed by dot, e.g. "81."
            if let Some(dot_idx) = rest.find('.') {
                let id_str = &rest[..dot_idx].trim();
                if let Ok(id) = id_str.parse::<u32>() {
                    let after_id = rest[dot_idx + 1..].trim();

                    // Parse [vol: 0.20 MUTED] or [vol: 0.20]
                    let (desc, vol, muted) = if let Some(bracket_start) = after_id.rfind('[') {
                        let desc = after_id[..bracket_start].trim().to_string();
                        let vol_part = after_id[bracket_start..].trim();
                        let is_muted = vol_part.contains("MUTED");

                        let volume = if let Some(vol_idx) = vol_part.find("vol:") {
                            let after_vol = vol_part[vol_idx + 4..].trim();
                            let num_str: String = after_vol
                                .chars()
                                .take_while(|c| c.is_ascii_digit() || *c == '.')
                                .collect();
                            num_str.parse::<f32>().unwrap_or(1.0)
                        } else {
                            1.0
                        };

                        (desc, volume, is_muted)
                    } else {
                        (after_id.to_string(), 1.0, false)
                    };

                    sinks.push(AudioDevice {
                        id,
                        name: format!("node-{}", id),
                        description: desc,
                        is_default,
                        volume: vol,
                        is_muted: muted,
                    });
                }
            }
        }
    }

    sinks
}

/// Query list of all audio sinks via wpctl
pub fn list_sinks() -> anyhow::Result<Vec<AudioDevice>> {
    let output = Command::new("wpctl")
        .arg("status")
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run wpctl status: {}", e))?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "wpctl status failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut sinks = parse_wpctl_sinks(&text);

    // If we have a default sink, let's get its precise node.name and node.description from wpctl inspect
    if let Ok(default_device) = get_default_sink() {
        for s in &mut sinks {
            if s.id == default_device.id || s.is_default {
                s.is_default = true;
                s.name = default_device.name.clone();
                if !default_device.description.is_empty() {
                    s.description = default_device.description.clone();
                }
            }
        }
    }

    Ok(sinks)
}

/// Query current default audio sink via `wpctl inspect @DEFAULT_AUDIO_SINK@`
pub fn get_default_sink() -> anyhow::Result<AudioDevice> {
    let output = Command::new("wpctl")
        .args(["inspect", "@DEFAULT_AUDIO_SINK@"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run wpctl inspect @DEFAULT_AUDIO_SINK@: {}", e))?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "wpctl inspect failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    parse_wpctl_inspect(&text)
}

/// Parses output of `wpctl inspect @DEFAULT_AUDIO_SINK@`
pub fn parse_wpctl_inspect(output: &str) -> anyhow::Result<AudioDevice> {
    let mut id = 0;
    let mut node_name = String::new();
    let mut description = String::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("id ") {
            // "id 81, type PipeWire:Interface:Node"
            if let Some(comma_pos) = trimmed.find(',') {
                let id_part = &trimmed[3..comma_pos].trim();
                id = id_part.parse().unwrap_or(0);
            }
        } else if trimmed.contains("node.name =")
            && let Some(val) = extract_quoted_value(trimmed)
        {
            node_name = val;
        } else if trimmed.contains("node.description =")
            && let Some(val) = extract_quoted_value(trimmed)
        {
            description = val;
        }
    }

    if description.is_empty() {
        description = if node_name.is_empty() {
            "Default Audio Output".to_string()
        } else {
            node_name.clone()
        };
    }

    // Try to get volume
    let (volume, is_muted) = get_default_volume().unwrap_or((1.0, false));

    Ok(AudioDevice {
        id,
        name: node_name,
        description,
        is_default: true,
        volume,
        is_muted,
    })
}

fn extract_quoted_value(line: &str) -> Option<String> {
    let start = line.find('"')?;
    let end = line[start + 1..].find('"')?;
    Some(line[start + 1..start + 1 + end].to_string())
}

/// Query current default audio sink volume via `wpctl get-volume @DEFAULT_AUDIO_SINK@`
pub fn get_default_volume() -> anyhow::Result<(f32, bool)> {
    let output = Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run wpctl get-volume: {}", e))?;

    let text = String::from_utf8_lossy(&output.stdout);
    // "Volume: 0.20 [MUTED]" or "Volume: 0.20"
    let is_muted = text.contains("[MUTED]");
    if let Some(vol_idx) = text.find("Volume:") {
        let rest = &text[vol_idx + 7..].trim();
        let num_str: String = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        let vol = num_str.parse::<f32>().unwrap_or(1.0);
        Ok((vol, is_muted))
    } else {
        Ok((1.0, is_muted))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_wpctl_sinks() {
        let sample = r#"
Audio
 ├─ Devices:
 │      48. AD107 High Definition Audio Controller [alsa]
 │      49. Built-in Audio                      [alsa]
 │      91. Chiv的AirPods Pro                  [bluez5]
 │  
 ├─ Sinks:
 │      33. Built-in Audio Analog Stereo        [vol: 0.00]
 │      44. AD107 High Definition Audio Controller Digital Stereo (HDMI) [AMZ G25F6B-1] [vol: 0.00]
 │  *   81. Chiv的AirPods Pro                  [vol: 0.20]
 │  
 ├─ Sources:
 │      46. Built-in Audio Analog Stereo        [vol: 1.00 MUTED]
"#;
        let sinks = parse_wpctl_sinks(sample);
        assert_eq!(sinks.len(), 3);
        assert_eq!(sinks[0].id, 33);
        assert!(!sinks[0].is_default);
        assert_eq!(sinks[0].description, "Built-in Audio Analog Stereo");

        assert_eq!(sinks[2].id, 81);
        assert!(sinks[2].is_default);
        assert_eq!(sinks[2].description, "Chiv的AirPods Pro");
        assert!((sinks[2].volume - 0.20).abs() < 0.001);
    }

    #[test]
    fn test_parse_wpctl_inspect() {
        let sample = r#"
id 81, type PipeWire:Interface:Node
    api.bluez5.address = "F4:33:B7:F3:A1:15"
  * client.id = "47"
  * media.class = "Audio/Sink"
  * node.description = "Chiv的AirPods Pro"
  * node.name = "bluez_output.F4_33_B7_F3_A1_15.1"
"#;
        let dev = parse_wpctl_inspect(sample).unwrap();
        assert_eq!(dev.id, 81);
        assert_eq!(dev.description, "Chiv的AirPods Pro");
        assert_eq!(dev.name, "bluez_output.F4_33_B7_F3_A1_15.1");
    }
}
