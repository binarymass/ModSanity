# ModSanity

<div align="center">

**A powerful, Terminal Based mod manager for Bethesda games on Linux**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Linux](https://img.shields.io/badge/Linux-FCC624?style=flat&logo=linux&logoColor=black)](https://www.linux.org/)

[Features](#features) • [Installation](#installation) • [Quick Start](#quick-start) • [Contributing](#contributing) • [Donate](https://buymeacoffee.com/tdpunkn0wnable)

[Release v0.1.6.5](https://github.com/binarymass/ModSanity/releases/tag/ModSanity)

**If you find this tool useful, consider showing your support with a donation. Obviously not a requirement, but a little goes a long way towards future development**

</div>

---

## Overview

ModSanity is a native Linux mod manager built from the ground up for Bethesda games. Designed for power users who prefer terminal workflows, it combines the full feature set of tools like Mod Organizer 2 and Vortex with the speed and efficiency of a Terminal TUI.

**Why ModSanity?**
- **Linux-native**: Built specifically for Linux with first-class Steam/Proton support
- **Terminal Based**: Complete TUI interface with vim-style navigation
- **Zero compromise**: Full FOMOD installer support, conflict detection, and profile management
- **Fast & efficient**: Written in Rust for maximum performance
- **Safe & deterministic**: Transactional deployments with automatic rollback
- **Just works**: Automatic game detection, intelligent mod categorization

---

## Features

### Game Management
- **Automatic game detection** for Steam libraries including Proton prefixes
- **Multi-game support** for Bethesda Creation Engine titles
- **Intelligent path resolution** for Windows games running under Proton
- Tested with Skyrim SE, Skyrim VR, and designed for Fallout 4, Oblivion, and more

### Advanced Mod Management
- **Full FOMOD installer support** with interactive wizard interface
  - Multi-step installations with conditional logic
  - Option flags and dependency resolution
  - Preview and rollback capabilities
  - Persistent installer choices per profile
- **Priority-based load ordering** with automatic conflict resolution
- **Smart conflict detection** showing file overwrites before deployment
- **Auto-categorization** using mod metadata and file structure analysis
- **Multiple deployment methods**: symlinks, hardlinks, or file copies
- **Archive support**: ZIP, 7-Zip with automatic extraction
- **Modlist save/load support**:
  - Save modlists in native JSON format or MO2-compatible format
  - In TUI Mods pane (`F1`), `L` opens saved modlists first
  - From the saved modlist picker, press `f` for file-path loading

### Profile System
- **Unlimited profiles** for different playthroughs or testing
- **Per-profile FOMOD installations** for variant mod setups
- **Instant switching** between mod configurations
- **Isolated plugin load orders** per profile

### Plugin Load Order Management
- **LOOT-compatible sorting** with masterlist integration
- **Dependency resolution** ensuring masters load correctly
- **Group-based ordering** for optimal plugin arrangement
- **Manual override support** for fine-tuning
- **ESP/ESM/ESL support** with proper flag handling

### NexusMods Integration
- **Direct mod browsing** from the TUI
- **Search functionality** across all game mods
- **One-click downloads** with progress tracking
- **Mod metadata sync** including descriptions and requirements
- **Requirement checking** via NexusMods API
- **Personal API key and SSO support** (experimental)

### Rich TUI Interface
- **Interactive terminal UI** built with Ratatui
- **Keyboard navigation** (vim-style keybindings supported)
- **Real-time status updates** during operations
- **Context-sensitive help** with `?` key
- **Multi-screen workflow**: Mods, Plugins, Downloads, Profiles, Settings, Import, Queue, Catalog, Modlists

### Developer-Friendly
- **CLI mode** for scripting and automation
- **JSON/YAML/TOML** configuration support
- **Comprehensive logging** with tracing support
- **SQLite database** for fast queries
- **Modular architecture** with clean APIs

---

## Installation

### From Source (Recommended)

```bash
# Clone the repository
git clone https://github.com/modsanity/modsanity.git
cd modsanity

# Build release binary
cargo build --release

# Install to ~/.local/bin (or your preferred location)
cp target/release/modsanity ~/.local/bin/

# Or run the install script
./install.sh
```

### Binary Releases

Download the latest binary from the [Releases](https://github.com/modsanity/modsanity/releases) page.

```bash
# Make executable
chmod +x modsanity

# Move to PATH
mv modsanity ~/.local/bin/
```

---

## Quick Start

### 1. Initial Setup

**Detect your games:**
```bash
modsanity game scan        # Scan Steam libraries
modsanity game list        # View detected games
modsanity game select skyrimse  # Set active game
```

### 2. Configure NexusMods (Optional)

**Option A - Personal API Key (for testing):**
```bash
modsanity nexus set-api-key YOUR_API_KEY
```

Get your API key at: https://www.nexusmods.com/users/myaccount?tab=api

**Option B - TUI Setup:**
1. Launch TUI: `modsanity`
2. Press `F4` for Settings
3. Select "NexusMods API Key"
4. Paste your key

### 3. Launch the TUI

```bash
modsanity
# or explicitly
modsanity tui
```

### 4. Install Your First Mod

**Via NexusMods (in TUI):**
1. Press `b` to browse NexusMods
2. Press `s` to search from the browse window
3. Select a mod and download
4. Press `i` to install from downloads

**Via Local Archive:**
```bash
modsanity mod install path/to/mod.zip
# or in TUI: press 'i' and select file
```

### 5. Deploy Mods

```bash
modsanity deploy
# or in TUI: press 'D' on the Mods screen
```

### 6. Configure Deployment Method (new)

```bash
modsanity deployment show
modsanity deployment set-method symlink
modsanity deployment set-method hardlink
modsanity deployment set-method copy

# one-time override per deploy
modsanity deploy --method hardlink
```

---

## TUI Keyboard Shortcuts

### Global Navigation
| Key | Action |
|-----|--------|
| `F1` | Mods screen |
| `F2` | Plugins screen |
| `F3` | Profiles screen |
| `F4` | Settings screen |
| `F5` | Import screen |
| `F6` | Queue screen |
| `F7` | Catalog screen |
| `F8` | Modlists screen |
| `g` | Game selection |
| `?` | Toggle help overlay |
| `q` / `Ctrl+C` | Quit |

### Mods Screen (F1)
| Key | Action |
|-----|--------|
| `↑/k` `↓/j` | Navigate mods |
| `Space` / `e` | Enable/disable mod |
| `+` / `=` | Increase priority (loads later) |
| `-` | Decrease priority |
| `i` | Install mod from archive |
| `l` | Load/install from Downloads folder |
| `S` | Save modlist |
| `L` | Load modlist (saved picker first) |
| `d` / `Delete` | Delete mod (with confirmation) |
| `D` | Deploy all enabled mods |
| `/` | Search/browse NexusMods |
| `r` | Refresh mod list |
| `x` | Check mod requirements |
| `o` | Open Load Order screen |

### Settings Screen (F4)
| Key | Action |
|-----|--------|
| `Enter` on Deployment Method | Cycle Symlink -> Hardlink -> Full Copy |
| `Enter` on Backup Originals | Toggle Yes/No |

---

## CLI Commands

### Game Management
```bash
modsanity game list           # List detected games
modsanity game scan           # Scan for games
modsanity game select <name>  # Select active game
modsanity game info           # Show active game info
```

### Mod Management
```bash
modsanity mod list                  # List installed mods
modsanity mod install <path>        # Install mod from archive
modsanity mod enable <name>         # Enable a mod
modsanity mod disable <name>        # Disable a mod
modsanity mod remove <name>         # Remove a mod
modsanity mod info <name>           # Show mod details
```

### Profile Management
```bash
modsanity profile list              # List profiles
modsanity profile create <name>     # Create new profile
modsanity profile switch <name>     # Switch to profile
modsanity profile delete <name>     # Delete profile
modsanity profile export <name> <path>  # Export profile
modsanity profile import <path>     # Import profile
```

### NexusMods
```bash
modsanity nexus set-api-key <key>   # Set personal API key
modsanity nexus login               # Login via SSO
modsanity nexus logout              # Logout
modsanity nexus search <query>      # Search for mods
modsanity nexus download <mod_id>   # Download a mod
modsanity nexus status              # Show account status
```

### Deployment
```bash
modsanity deployment show                        # Show deployment settings
modsanity deployment set-method <symlink|hardlink|copy>
modsanity deploy                                 # Deploy mods to game directory
modsanity deploy --method <symlink|hardlink|copy>
modsanity status                                 # Show current status
```

---

## FOMOD Installer Support

ModSanity includes full MO2/Vortex-grade FOMOD support, built from the ground up for Linux.

### Features
- **Complete XML parsing** of `ModuleConfig.xml` and `info.xml`
- **Multi-step installations** with pages and groups
- **Conditional logic** with flag evaluation
- **Requirement validation** (SelectExactlyOne, SelectAtMostOne, etc.)
- **File mapping** and custom install paths
- **Conflict preview** before installation
- **Transactional execution** with automatic rollback on failure
- **Persistent choices** - rerun installers with saved selections
- **Per-profile installations** - different FOMOD configs per profile

For details, see [FOMOD_USER_GUIDE.md](docs/FOMOD_USER_GUIDE.md).

---

## Configuration

### Config Location
- **Config file**: `~/.config/modsanity/config.toml`
- **Data directory**: `~/.local/share/modsanity/`
- **Mod storage**: `~/.local/share/modsanity/games/<game_id>/mods/`

### Deployment Methods
Configure in `config.toml`:
```toml
[deployment]
method = "symlink"  # Options: "symlink", "hardlink", "copy"
backup_originals = false
```

- **symlink**: Fast, space-efficient (recommended)
- **hardlink**: Useful when symlink behavior is restricted
- **copy**: Full file copies (most compatible, uses more space)

---

## Supported Games

Currently tested with:
- **The Elder Scrolls V: Skyrim Special Edition**
- **The Elder Scrolls V: Skyrim VR**

Designed to work with all Creation Engine games:
- Fallout 4
- Fallout: New Vegas
- The Elder Scrolls IV: Oblivion
- Fallout 3
- Starfield

---

## Contributing

Contributions are welcome.

### Development Setup
```bash
git clone https://github.com/modsanity/modsanity.git
cd modsanity
cargo build
cargo test
```

### Code Style
- Follow Rust conventions (`cargo fmt`, `cargo clippy`)
- Add tests for new features
- Update documentation for user-facing changes

---

## License

ModSanity is licensed under the [MIT License](LICENSE).
