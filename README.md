# ModSanity

A CLI/TUI mod manager for Bethesda games on Linux.

[Release v0.1.7](https://github.com/binarymass/ModSanity/releases/tag/ModSanity)

## Overview

ModSanity provides:
- Game detection for supported Steam installs.
- GOG/manual game path registration and detection.
- Mod install/remove/enable/disable workflows.
- Deployment to game directories (symlink, hardlink, or copy).
- FOMOD support with interactive wizard in the TUI.
- Plugin/load-order management.
- Nexus catalog population + mod browsing/import tooling.
- Queue-based download/install processing.
- Database-backed modlists.
- External tool launch via Proton.

## Verified Features (from current code)

### Game and environment
- Automatic Steam library scanning and game detection.
- Proton prefix detection via `steamapps/compatdata/<app_id>`.
- Active game selection persisted in config.

### Mod management
- Install mods from archives (`.zip`, `.7z`, `.rar`).
- Remove, enable, disable, list, and inspect installed mods.
- Priority-based conflict resolution during deployment.
- Case-insensitive path normalization during deployment to avoid duplicate folder casing splits.
- Deployment methods: `symlink`, `hardlink`, `copy`.
- SKSE override behavior:
  - SKSE runtime binaries (`skse*.exe`, `skse*.dll`) are deployed next to the game executable.
  - SKSE-related files are always hard-copied (never linked), regardless of global deploy method.
- Rescan staging directory to add/update existing mods in DB, re-index files/plugins, and report added/updated/unchanged/failed stats.

### FOMOD
- FOMOD detection and parsing (`ModuleConfig.xml` / `info.xml` handling, case-insensitive search).
- Interactive TUI wizard flow for option selection and conditional installs.
- FOMOD plan persistence support in DB.
- CLI install path explicitly fails when a wizard is required (TUI required for interactive FOMOD).

### Plugins and load order
- Plugin scanning (`.esp`, `.esm`, `.esl`) from game `Data`.
- Read/write `plugins.txt` and `loadorder.txt` (Proton AppData paths).
- Manual reorder and save from TUI.
- Native Rust auto-sort.
- Optional LOOT CLI sort if LOOT executable is available.

### Profiles
- Create/list/switch/delete profiles.
- Export/import profile files.

### Modlists and import
- Save modlists to file:
  - Native JSON format.
  - MO2-style text format.
- Load modlists from file (native and MO2 paths).
- Persist saved/imported modlists in SQLite (`modlists` + `modlist_entries`).
- TUI modlist editor for saved modlists (create/rename/delete modlists, enable/disable/reorder/delete entries).
- Import matching pipeline with DB catalog support and plugin-name-assisted matching.
- MO2 migration bridge command to apply plugin enabled/disabled state to installed mods.

### Nexus integration
- Local Nexus catalog population (REST-backed) and resume/status tracking.
- TUI browse/search with sort and pagination, file selection, and queueing.
- Requirement checks for selected mods in TUI (API key required).

### Queue system
- Persistent queue entries in DB.
- Batch processing with concurrent downloads.
- Optional download-only mode.
- Retry failed items and clear batch.

### External tools (Proton)
- Selectable Steam-managed Proton runtime detection (`steamapps/common` and `compatibilitytools.d`).
- Optional custom Proton command/path fallback.
- Per-tool runtime mode override (`proton` or `native`).
- Configurable tool executable paths for:
  - xEdit, SSEEdit, FNIS, Nemesis, Symphony, BodySlide, Outfit Studio.
- Launch configured tools via Proton from CLI or TUI Settings.

### Configurable storage paths
- Configurable downloads directory override.
- Configurable staging/installed-mods directory override.
- Both are configurable from CLI and TUI Settings.

## Supported Games

Current `GameType` implementations:
- Skyrim Special Edition (`skyrimse`)
- Skyrim VR (`skyrimvr`)
- Fallout 4 (`fallout4`)
- Fallout 4 VR (`fallout4vr`)
- Starfield (`starfield`)

## Requirements

- Linux
- Rust toolchain (for source builds)
- Optional for `.rar` extraction: `unrar`
- Optional for LOOT sort: `loot` executable
- Nexus API key for Nexus features (browse/import/download/catalog populate)

## Installation

### Build from source

```bash
git clone https://github.com/modsanity/modsanity.git
cd modsanity
cargo build --release
cp target/release/modsanity ~/.local/bin/
```

### Install script

```bash
./install.sh
```

## Configuration

XDG-backed paths used by the app:
- Config: `~/.config/modsanity/config.toml`
- Data: `~/.local/share/modsanity/`
- Cache: `~/.cache/modsanity/`

Important config keys:
- `active_game`
- `active_profile`
- `nexus_api_key`
- `[deployment]` with `method`, `backup_originals`, `purge_on_exit`
- `downloads_dir_override`
- `staging_dir_override`
- `[external_tools]` with `proton_command`, optional `proton_runtime`, and tool paths

Example deployment config:

```toml
[deployment]
method = "symlink"   # symlink | hardlink | copy
backup_originals = true
purge_on_exit = false
```

## Quick Start

```bash
# 1) Detect and select game
modsanity game scan
modsanity game list
modsanity game select skyrimse

# 2) Install a mod archive
modsanity mod install /path/to/mod.zip

# 3) Deploy
modsanity deploy

# 4) Check status
modsanity status
```

For interactive workflows (FOMOD wizard, browse, queue review, modlist editor), launch:

```bash
modsanity
```

Extended guides:
- `docs/quickstart.md`
- `docs/migration/mo2.md`
- `docs/ui/accessibility.md`
- `CLI-Documentation.md`
- `docs/verification/questions_phase_status.md`

## CLI Command Reference

### Top-level
- `modsanity` (launch TUI)
- `modsanity --mods-dir <path> <command...>` (runtime staging override)
- `modsanity tui`
- `modsanity status`
- `modsanity deploy [--method symlink|hardlink|copy]`
- `modsanity doctor [--verbose]`
- `modsanity init [--game-id ... --platform ... --game-path ... --downloads-dir ... --staging-dir ... --proton-prefix ...]`
- `modsanity audit --dry-run`
- `modsanity getting-started`

### Game
- `modsanity game list`
- `modsanity game scan`
- `modsanity game select <name>`
- `modsanity game info`
- `modsanity game add-path <game_id> <path> [--platform steam|gog|manual] [--proton-prefix <path>]`
- `modsanity game remove-path <game_id> <path>`

### Mod
- `modsanity mod list`
- `modsanity mod install <path>`
- `modsanity mod enable <name>`
- `modsanity mod disable <name>`
- `modsanity mod remove <name>`
- `modsanity mod info <name>`
- `modsanity mod rescan`

### Profile
- `modsanity profile list`
- `modsanity profile create <name>`
- `modsanity profile switch <name>`
- `modsanity profile delete <name>`
- `modsanity profile export <name> <path>`
- `modsanity profile import <path>`

### Import
- `modsanity import modlist <path> [--auto-approve] [--preview]`
- `modsanity import status <batch_id>`
- `modsanity import apply-enabled <path> [--preview]`

### Queue
- `modsanity queue list`
- `modsanity queue process --batch-id <id> [--download-only]`
- `modsanity queue retry`
- `modsanity queue clear --batch-id <id>`

### Modlist
- `modsanity modlist save <path> [--format native|mo2]`
- `modsanity modlist load <path> [--auto-approve] [--preview]`

### Nexus catalog
- `modsanity nexus populate --game <domain> [--reset] [--per-page N] [--max-pages N]`
- `modsanity nexus status --game <domain>`

### Deployment settings
- `modsanity deployment show`
- `modsanity deployment set-method <symlink|hardlink|copy>`
- `modsanity deployment set-downloads-dir <path>`
- `modsanity deployment clear-downloads-dir`
- `modsanity deployment set-staging-dir <path>`
- `modsanity deployment clear-staging-dir`
- `modsanity deployment migrate-staging <from> <to> [--dry-run]`

### External tools
- `modsanity tool show`
- `modsanity tool list-proton`
- `modsanity tool use-proton <runtime-id|auto>`
- `modsanity tool clear-proton-runtime`
- `modsanity tool set-proton <path-or-command>`
- `modsanity tool set-path <tool> <path>`
- `modsanity tool set-runtime <tool> <proton|native>`
- `modsanity tool clear-runtime <tool>`
- `modsanity tool clear-path <tool>`
- `modsanity tool run <tool> [-- <args...>]`

Tool IDs:
- `xedit`, `ssedit`/`sseedit`, `fnis`, `nemesis`, `symphony`, `bodyslide`, `outfitstudio`

## TUI Screens (current)

Function keys:
- `F1` Mods
- `F2` Plugins
- `F3` Profiles
- `F4` Settings
- `F5` Import
- `F6` Queue
- `F7` Nexus Catalog
- `F8` Modlists

Global keys:
- `?` help
- `g` game select
- `q` quit

Help overlay:
- `?` opens/closes the full help overlay.
- Help is paginated and includes TUI keybindings plus a CLI command map.
- Navigate help pages with `n`/Right/`PgDn` and `p`/Left/`PgUp`.

Settings notes:
- Deployment method, backup toggle, API key, default mod directory, downloads/staging overrides.
- Proton runtime selection, Proton command, and external tool paths are editable.
- `l` launches the selected tool when a tool-path row is selected.

## Known Behavioral Notes

- Nexus-powered flows require a configured `nexus_api_key`.
- CLI install cannot complete interactive FOMOD wizards; use TUI for those installs.
- RAR extraction requires `unrar` on the system.

## Development

```bash
cargo build
cargo test
cargo build --release
```

## License

MIT (`LICENSE`)
