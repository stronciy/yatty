<div align="center">

# yatty

**Yet Another TeleTYpewriter**

A fast, minimal, GPU-accelerated terminal emulator for macOS on Apple Silicon, written in Rust.

[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
![Platform](https://img.shields.io/badge/platform-macOS%20(Apple%20Silicon)-black.svg)
![Language](https://img.shields.io/badge/language-Rust-orange.svg)
![Status](https://img.shields.io/badge/status-early%20development-yellow.svg)

</div>

---

> **Status: early development.** yatty is not yet ready for daily use. The core terminal, configuration, settings window and footer are working. Tabs, split panes and search are planned.

## Features

- **Native and fast.** GPU text rendering with [wgpu](https://wgpu.rs) (Metal backend) and glyph atlas caching.
- **Proper text shading.** Font shaping with [cosmic-text](https://github.com/pop-os/cosmic-text), including programming ligatures (`->`, `=>`, `!=`) and any installed monospace font.
- **Full terminal emulation.** Built on [`alacritty_terminal`](https://github.com/alacritty/alacritty), with true color, 256-color and ANSI support, and a real PTY for your shell.
- **Mouse selection and clipboard.** Select text with the mouse, copy with `⌘C`, paste with `⌘V`. Shortcuts work with any keyboard layout.
- **Configurable from a file and from the UI.** A TOML configuration file, named themes, and a native **Menu → Settings…** window to change fonts, colors and the footer.
- **Informative footer.** Shows encoding, shell, current folder, git branch, time and terminal size.
- **oh-my-zsh style prompt.** For zsh, yatty provides a prompt in the style of the popular *robbyrussell* theme without requiring oh-my-zsh. Your own `~/.zshrc` is still loaded.
- **Native macOS integration.** Standard application menu, Apple Silicon build, no Electron or web runtime.

## Requirements

- macOS on Apple Silicon (`aarch64`). Intel Macs are not supported.
- A recent stable Rust toolchain (Rust 1.88 or newer). Install it with [rustup](https://rustup.rs).
- A monospace font. [JetBrains Mono](https://www.jetbrains.com/lp/mono/) is used by default; any installed monospace font can be selected in the settings.

## Getting started

### Build and run

```sh
git clone https://github.com/stronciy/yatty.git
cd yatty
cargo run --release -p app
```

### Run the tests

```sh
cargo test --workspace
```

## Configuration

yatty reads its configuration from `~/.config/yatty/config.toml`. Every option is optional; without a file, defaults are used. A fully commented example is available in [`docs/config.example.toml`](docs/config.example.toml).

```toml
[font]
family = "JetBrains Mono"
size = 14.0
weight = 400          # 400 = Regular, 700 = Bold
ligatures = true

[colors]
theme = "default"     # or the name of a file in ~/.config/yatty/themes/

[footer]
enabled = true

[prompt]
enabled = true        # oh-my-zsh style prompt for zsh
```

### Themes

A theme is a TOML file in `~/.config/yatty/themes/<name>.toml` with the same keys as the `[colors]` section. A sample is available in [`docs/theme.example.toml`](docs/theme.example.toml).

Settings are applied in this order: built-in default theme, then the named theme file, then inline overrides in `[colors]`.

## Keyboard and mouse

| Action | Shortcut |
|---|---|
| Settings | `⌘,` (Menu → Settings…) |
| Select text | Click and drag with the left mouse button |
| Copy selection | `⌘C` |
| Paste | `⌘V` |
| Quit | `⌘Q` |

## Architecture

yatty is organized as a Cargo workspace with five crates. Dependencies flow downward and there are no cycles.

| Crate | Responsibility |
|---|---|
| [`term`](crates/term) | PTY, shell process, terminal state via `alacritty_terminal`, selection |
| [`render`](crates/render) | wgpu renderer, glyph atlas, text shaping |
| [`config`](crates/config) | TOML configuration, themes, settings persistence |
| [`ui`](crates/ui) | Reserved for tabs and split panes (not yet implemented) |
| [`app`](crates/app) | Window and event loop (winit), menu (muda), settings window (egui), footer, shell integration |

Design notes and specifications (in Russian) live in [`docs/superpowers/`](docs/superpowers).

## Roadmap

- [x] Single-window terminal with PTY shell and true color
- [x] Font, theme and footer configuration; Settings window
- [x] Mouse selection, copy and paste
- [x] Footer with encoding, shell, folder, git branch, time and size
- [x] oh-my-zsh style prompt for zsh
- [ ] Bold, italic and underline rendering
- [ ] Tabs (with inherited working directory)
- [ ] Split panes
- [ ] Scrollback search
- [ ] Mouse reporting for terminal applications
- [ ] Live reload of the configuration file
- [ ] Signed and notarized `.dmg` and a Homebrew formula

## Contributing

Contributions are welcome. Please open an issue to discuss larger changes before starting work on them. Before submitting a pull request, make sure that:

```sh
cargo fmt --all
cargo clippy --workspace
cargo test --workspace
```

all pass without warnings.

## Acknowledgements

yatty builds on excellent open-source work, including [alacritty_terminal](https://github.com/alacritty/alacritty), [wgpu](https://wgpu.rs), [cosmic-text](https://github.com/pop-os/cosmic-text), [winit](https://github.com/rust-windowing/winit), [egui](https://github.com/emilk/egui) and [muda](https://github.com/tauri-apps/muda). The oh-my-zsh prompt is inspired by [oh-my-zsh](https://ohmyz.sh) and its *robbyrussell* theme.

## License

yatty is licensed under the [Apache License, Version 2.0](LICENSE).
