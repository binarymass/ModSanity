# ModSanity

<div align="center">

**A powerful, keyboard-first mod manager for Bethesda games on Linux**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Linux](https://img.shields.io/badge/Linux-FCC624?style=flat&logo=linux&logoColor=black)](https://www.linux.org/)

[Features](#-features) • [Installation](#-installation) • [Quick Start](#-quick-start) • [Documentation](#-documentation) • [Contributing](#-contributing) • [Donate](https://buymeacoffee.com/tdpunkn0wnable)

**If you find this tool useful, consider showing your support with a donation. Obviously not a requirement, but a little goes a long way towards future development**

</div>

---

## 🎯 Overview

ModSanity is a native Linux mod manager built from the ground up for Bethesda games. Designed for power users who prefer terminal workflows, it combines the full feature set of tools like Mod Organizer 2 and Vortex with the speed and efficiency of a keyboard-driven TUI.

**Why ModSanity?**
- 🐧 **Linux-native**: Built specifically for Linux with first-class Steam/Proton support
- ⌨️ **Keyboard-first**: Complete TUI interface with vim-style navigation
- 🎯 **Zero compromise**: Full FOMOD installer support, conflict detection, and profile management
- 🚀 **Fast & efficient**: Written in Rust for maximum performance
- 🔒 **Safe & deterministic**: Transactional deployments with automatic rollback
- 🎮 **Just works**: Automatic game detection, intelligent mod categorization

---

## ✨ Features

### 🎮 Game Management
- **Automatic game detection** for Steam libraries including Proton prefixes
- **Multi-game support** for Bethesda Creation Engine titles
- **Intelligent path resolution** for Windows games running under Proton
- Tested with Skyrim SE, Skyrim VR, and designed for Fallout 4, Oblivion, and more

### 📦 Advanced Mod Management
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

### 👤 Profile System
- **Unlimited profiles** for different playthroughs or testing
- **Per-profile FOMOD installations** for variant mod setups
- **Instant switching** between mod configurations
- **Isolated plugin load orders** per profile

### 🧩 Plugin Load Order Management
- **LOOT-compatible sorting** with masterlist integration
- **Dependency resolution** ensuring masters load correctly
- **Group-based ordering** for optimal plugin arrangement
- **Manual override support** for fine-tuning
- **ESP/ESM/ESL support** with proper flag handling

### 🌐 NexusMods Integration
- **Direct mod browsing** from the TUI
- **Search functionality** across all game mods
- **One-click downloads** with progress tracking
- **Mod metadata sync** including descriptions and requirements
- **Requirement checking** via NexusMods API
- **Personal API key and SSO support** (experimental)

### 🎨 Rich TUI Interface
- **Interactive terminal UI** built with Ratatui
- **Keyboard-driven navigation** (vim-style keybindings supported)
- **Real-time status updates** during operations
- **Context-sensitive help** with `?` key
- **Multi-screen workflow**: Mods, Plugins, Downloads, Profiles, Settings

### 🔧 Developer-Friendly
- **CLI mode** for scripting and automation
- **JSON/YAML/TOML** configuration support
- **Comprehensive logging** with tracing support
- **SQLite database** for fast queries
- **Modular architecture** with clean APIs

---

## 📥 Installation

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

## 🚀 Quick Start

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
1. Press `/` to search NexusMods
2. Select a mod and download
3. Press `i` to install from downloads

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

---

## ⌨️ TUI Keyboard Shortcuts

### Global Navigation
| Key | Action |
|-----|--------|
| `F1` | Mods screen |
| `F2` | Plugins screen |
| `F3` | Profiles screen |
| `F4` | Settings screen |
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
| `d` / `Delete` | Delete mod (with confirmation) |
| `D` | Deploy all enabled mods |
| `/` | Search/browse NexusMods |
| `r` | Refresh mod list |
| `x` | Check mod requirements |

### Plugins Screen (F2)
| Key | Action |
|-----|--------|
| `Space` / `e` | Enable/disable plugin |
| `s` | Save and optimize load order |
| `↑/k` `↓/j` | Navigate plugins |

### Profiles Screen (F3)
| Key | Action |
|-----|--------|
| `Enter` | Switch to profile |
| `n` | Create new profile |
| `d` / `Delete` | Delete profile |

### Browse/Downloads Screen 
| Key | Action |
|-----|--------|
| `Enter` | Download selected mod |
| `/` | Search NexusMods |
| `r` | Refresh/load top mods |

---

## 🖥️ CLI Commands

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
modsanity deploy                    # Deploy mods to game directory
modsanity status                    # Show current status
```

---

## 📚 Documentation

- **[FOMOD User Guide](docs/FOMOD_USER_GUIDE.md)** - Complete FOMOD installer documentation
- **[FOMOD Architecture](docs/FOMOD_ARCHITECTURE.md)** - Technical details of FOMOD implementation
- **[Packaging Guide](PACKAGING.md)** - How to create releases
- **[Changelog](CHANGELOG.md)** - Version history and updates

---

## 🎯 FOMOD Installer Support

ModSanity includes **full MO2/Vortex-grade FOMOD support**, built from the ground up for Linux:

### Features
- ✅ **Complete XML parsing** of `ModuleConfig.xml` and `info.xml`
- ✅ **Multi-step installations** with pages and groups
- ✅ **Conditional logic** with flag evaluation
- ✅ **Requirement validation** (SelectExactlyOne, SelectAtMostOne, etc.)
- ✅ **File mapping** and custom install paths
- ✅ **Conflict preview** before installation
- ✅ **Transactional execution** with automatic rollback on failure
- ✅ **Persistent choices** - rerun installers with saved selections
- ✅ **Per-profile installations** - different FOMOD configs per profile

### Usage
1. Install a FOMOD-enabled mod
2. ModSanity auto-detects the installer
3. Launch the interactive wizard (in TUI or via CLI)
4. Navigate through steps, select options
5. Preview file operations and conflicts
6. Confirm and install with rollback safety

For details, see [FOMOD_USER_GUIDE.md](docs/FOMOD_USER_GUIDE.md).

---

## 🏗️ Architecture Highlights

### Safe & Deterministic
- **Transactional deployments**: All-or-nothing installations with automatic rollback
- **Priority-based conflict resolution**: Explicit, reproducible load orders
- **No hidden state**: Always preview what files will be deployed
- **Atomic operations**: Safe profile switching and mod updates

### Performance Optimized
- **Async I/O**: Fast downloads and file operations
- **SQLite caching**: Quick queries for large mod lists
- **Efficient algorithms**: Topological sorting for plugin load orders
- **Minimal overhead**: Direct symlinks avoid file duplication

### Modular Design
```
src/
├── games/       # Game detection and configuration
├── mods/        # Mod management and FOMOD
├── plugins/     # Plugin load order and LOOT integration
├── profiles/    # Profile management
├── nexus/       # NexusMods API client
├── db/          # Database layer
├── config/      # Configuration handling
└── tui/         # Terminal UI
```

---

## 🔧 Configuration

### Config Location
- **Config file**: `~/.config/modsanity/config.toml`
- **Data directory**: `~/.local/share/modsanity/`
- **Mod storage**: `~/.local/share/modsanity/games/<game_id>/mods/`

### Deployment Methods
Configure in `config.toml`:
```toml
[deployment]
method = "symlink"  # Options: "symlink", "hardlink", "copy"
```

- **symlink**: Fast, space-efficient (recommended)
- **hardlink**: Safe for games that follow symlinks
- **copy**: Full file copies (most compatible, uses more space)

---

## 🎮 Supported Games

Currently tested with:
- ✅ **The Elder Scrolls V: Skyrim Special Edition**
- ✅ **The Elder Scrolls V: Skyrim VR**

Designed to work with all Creation Engine games:
- 🔄 Fallout 4
- 🔄 Fallout: New Vegas
- 🔄 The Elder Scrolls IV: Oblivion
- 🔄 Fallout 3
- 🔄 Starfield

*Community testing and contributions welcome for additional games!*

---

## 🐛 Known Limitations

- **NexusMods SSO**: Requires official app registration (use personal API key for now)
- **Windows-only mods**: Some tools with native Windows executables won't work (use Proton workarounds)
- **Early development**: Some edge cases may not be handled perfectly

---

## 🤝 Contributing

Contributions are welcome! Here's how you can help:

1. **Report bugs**: Open an issue with details and reproduction steps
2. **Suggest features**: Describe your use case and desired functionality
3. **Test new games**: Try ModSanity with other Bethesda titles and report results
4. **Submit PRs**: Fix bugs, add features, or improve documentation

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

## 📜 License

ModSanity is licensed under the [MIT License](LICENSE).

---

## 🙏 Acknowledgments

- **LOOT** - Load order optimization algorithms and masterlist format
- **Mod Organizer 2** - FOMOD specification and mod management patterns
- **Ratatui** - Excellent terminal UI framework
- **NexusMods** - API and mod hosting infrastructure
- **Bethesda modding community** - For decades of incredible content

---

## 💬 Support & Community

- **Issues**: [GitHub Issues](https://github.com/modsanity/modsanity/issues)
- **Discussions**: [GitHub Discussions](https://github.com/modsanity/modsanity/discussions)

---

## 🗺️ Roadmap

### Planned Features
- [ ] Collections support (Nexus Collections integration)
- [ ] Download queue management
- [ ] Mod update notifications
- [ ] BSA/BA2 archive extraction
- [ ] Integrated conflict resolver UI
- [ ] Web UI (optional companion to TUI)
- [ ] Cloud profile sync
- [ ] Mod dependency graph visualization
- [ ] xEdit integration for conflict detection
- [ ] SKSE/F4SE version management

### Long-term Goals (Function Before Finesse)
- GUI version (GTK/Qt)
- Windows/macOS support
- Integration with other mod sites (ModDB, LoversLab, etc.)
- Advanced scripting and automation APIs

---

<div align="center">

**Made with ❤️ for the Linux modding community**

[⬆ Back to Top](#modsanity)

</div>
