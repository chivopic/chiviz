<div align="center">

![chiviz banner](assets/chiviz-banner.svg)

# chiviz

**Audio Spectrum Visualizer & Omarchy Status Bar Plugin**  
*Optimized for Omarchy, Kitty Terminal, Hyprland, and PipeWire.*

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust Edition](https://img.shields.io/badge/Rust-2024_Edition-orange.svg)](Cargo.toml)
[![Omarchy Plugin](https://img.shields.io/badge/Omarchy_Plugin-v1_Compliant-10b981.svg)](manifest.json)
[![Platform](https://img.shields.io/badge/Platform-Omarchy_%7C_Arch_Linux-blueviolet.svg)](https://omarchy.org)

[English](README.md) · [简体中文](README_zh.md)

</div>

---

`chiviz` is a high-performance terminal audio spectrum visualizer and Omarchy shell plugin. It passively captures system PipeWire output (Spotify, browser, games) and renders real-time audio-reactive animations in Kitty and Hyprland, automatically synchronized with the active Omarchy system theme.

## Visual Modes

| Mode | Key | Collection | Description |
|:---|:---:|:---|:---|
| **Classic Bars** | `1` | Retro | High-density spectrum bars with floating peak caps and 1/8th Unicode sub-blocks |
| **Mirror Spectrum** | `2` | Retro | Symmetrical top-bottom reflection with animated water-surface ripples and depth fading |
| **Smooth Wave** | `3` | Modern | Continuous multi-harmonic waveform with dual-layer echo and 2×4 Braille sub-pixel matrices |
| **Orbit** | `4` | Modern | Circular radial spectrum with pulsating reactor core and orbiting satellite particles |

### 1. Classic Bars
```text
  80 dB ┤                                        ▔
        │                        ▔       █             ▔
        │                █       █       █       █     █
        │        █       █   █   █   █   █   █   █ █   █   █
        │    █   █ █ █   █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █ █
   0 dB ┴────┴───┴─┴─┴───┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴───
        50Hz        250Hz        1kHz         4kHz        16kHz
```

### 2. Mirror Spectrum
```text
        │        █       █       █       █       █
        │    █   █ █ █   █ █ █ █ █ █ █ █ █ █ █ █ █ █ █
  ──────┼────┴───┴─┴─┴───┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴────────── (Centerline)
        │    ░   ▒ ░ ░   █ ▒ ░ ▒ █ ▒ ░ ▒ █ ▒ ░ ▒ █ ░ ░
        │        ░       ▒       ▒       ░       ░
```

### 3. Smooth Wave
```text
        ⡀                                   ⢄
       ⢠⠃⠣⡀                                ⠳⡄
      ⢠⠃   ⠱⡄     ⢠⠞⠙⣆                    ⠹⣄
  ───⠤⠊───────⠘⠢⠤⠤⠞─────⠈⠢⠤⠤⠖⠊⠑⠢⠤⠤⠴⠒⠊⠉⠙⠢⠤⠴⠒⠊⠉⠙⠒─────
```

### 4. Orbit
```text
                          ⠘⡆ ⡞⠃
                     ⢀⠤⠒⠈⠉⠁⠒⠤⡀
                  ⡠⠊   ⢀⠤⠒⠤⡀   ⠑⠢⢄
                ⢰⠁    ⢎  ⚡  ⡱     ⠈⡆
                ⠈⡄    ⠈⠢⠤⠔⠁     ⢠⠃
                  ⠑⠤⣀   ⣀⠤⠒⠤⣀   ⢀⠤⠊
                     ⠈⠉⠁    ⠈⠉⠁
                           ⠱⡄ ⢠⠎
```

## Architecture & Data Flow

![Chiviz Architecture](assets/chiviz-architecture.svg)

- **PipeWire Capture**: Intercepts output sink monitors passively without recording microphones or requesting extra permissions.
- **Dynamic Failover**: Automatically tracks audio sink changes (Headphones ⇄ Internal Speakers) with instant recovery.
- **Motion Engine**: Instant attack, exponential release, floating peak hold, and non-linear perceptual loudness scaling ($x^{0.72}$).
- **Omarchy Theme Sync**: Live-monitors `~/.local/state/omarchy/current/theme/colors.toml` and hot-reloads palettes.
- **Diagnostics**: Built-in `chiviz --doctor` verifies Kitty terminal, PipeWire, CAVA streams, and theme paths.

## Installation

The marketplace installs the bar widget files; it does not build or install the `chiviz` executable. Install the runtime dependencies and build the executable before enabling the widget. The same binary setup is required when using Chiviz from the terminal without the bar widget.

### 1. Install dependencies and build Chiviz

On Omarchy / Arch Linux, install the Rust toolchain and runtime dependencies (`rust` provides Cargo):

```bash
sudo pacman -S --needed rust cava pipewire wireplumber kitty
```

Build and install the executable to `~/.local/bin`:

```bash
git clone https://github.com/chivopic/chiviz.git
cd chiviz
cargo build --release
mkdir -p ~/.local/bin
install -m 755 target/release/chiviz "$HOME/.local/bin/chiviz"
```

Make sure `~/.local/bin` is present in `PATH` for both your terminal and the Omarchy desktop session, then open a new terminal/session. Verify the binary and dependencies with:

```bash
command -v chiviz
chiviz --doctor
```

### 2. Use from the terminal

After the build above, launch Chiviz directly:

```bash
chiviz --demo
```

This terminal-only use does not require installing or enabling the bar plugin.

### 3. Install and enable the bar plugin

After `chiviz` is installed and on the desktop session `PATH`, install and enable the plugin:

```bash
omarchy plugin add https://github.com/chivopic/chiviz.git --enable
```

The left, middle, and right bar actions launch the visualizer, floating mode, and `chiviz --doctor` respectively. If `chiviz` or Kitty is missing from `PATH`, the widget shows an Omarchy notification with the next setup command instead of silently failing.

### Uninstall

Remove the bar plugin with:

```bash
omarchy plugin remove chiv.chiviz --yes
```

The plugin removal does not uninstall the manually built binary. To remove that binary separately, run:

```bash
rm -f ~/.local/bin/chiviz
```

## Keybindings

| Key | Action |
|:---|:---|
| `Space` / `m` / `n` | Cycle visual mode (displays top toast notification) |
| `1` – `4` | Direct switch to Classic, Mirror, Wave, or Orbit |
| `p` | Cycle previous visual mode |
| `Tab` / `Shift+Tab` | Cycle next / previous color theme |
| `f` | Toggle between Immersive and Compact layouts |
| `+` / `-` | Adjust sensitivity multiplier |
| `s` | Freeze / resume animation |
| `?` / `h` | Toggle keyboard help modal |
| `q` / `Ctrl+C` | Quit |

## CLI Options

```bash
chiviz                         # Default launch (auto-detects PipeWire sink & Omarchy theme)
chiviz --mode wave             # Launch specific mode (classic, mirror, wave, orbit)
chiviz --theme tokyonight      # Launch with theme override
chiviz --demo                  # Test visualization with synthetic audio
chiviz --doctor                # Run environment diagnostics
chiviz --list-devices          # List all available PipeWire audio sinks
```

## Hyprland Integration

Add to `~/.config/hypr/rules.conf`:

```ini
windowrulev2 = float, class:^(chiviz-visualizer|chiviz-float)$
windowrulev2 = size 920 480, class:^(chiviz-visualizer|chiviz-float)$
windowrulev2 = center, class:^(chiviz-visualizer|chiviz-float)$
```

## License

MIT License. See [LICENSE](LICENSE) for details.
