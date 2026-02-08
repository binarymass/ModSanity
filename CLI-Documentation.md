# ModSanity CLI Documentation

This document is a complete usage guide for the currently implemented ModSanity CLI (`v0.1.7`).
It is based on the live command surface (`modsanity --help` and subcommand help) and command
handler behavior in `src/main.rs` and `src/app/actions.rs`.

## 1. Command Model

Top-level usage:

```bash
modsanity [OPTIONS] [COMMAND]
```

Top-level commands:

- `tui`
- `game`
- `mod`
- `profile`
- `import`
- `queue`
- `modlist`
- `nexus`
- `deployment`
- `tool`
- `deploy`
- `status`
- `doctor`
- `init`
- `audit`
- `getting-started`

Top-level options:

- `-b, --batch`
- `-v, --verbose` (repeatable: `-v`, `-vv`, `-vvv`)
- `--mods-dir <PATH>` (runtime staging/mods directory override for this invocation)

## 2. Global Prerequisites and Conventions

Common preconditions used by many commands:

- An active game is required for most mod/profile/import/queue operations.
- Nexus API key is required for Nexus-powered flows (import matching, catalog populate, queue downloads).
- External tools require:
  - configured tool executable path (`tool set-path`)
  - active game with a usable Proton prefix
  - usable Proton runtime/launcher configuration

Path conventions:

- Config is loaded from XDG-backed app config path (see `README.md` for locations).
- Downloads/staging paths are resolved from config (overrides if set).

### Recommended baseline workflow

```bash
modsanity init --interactive
modsanity doctor --verbose
modsanity game list
modsanity game select skyrimse
```

## 3. Top-Level Commands

### `modsanity` / `modsanity tui`
Launches the interactive TUI.

Usage:

```bash
modsanity
modsanity tui
```

### `modsanity status`
Prints a short status summary:

- active game
- active profile
- deployment mode
- installed/enabled mod counts (if active game exists)

Usage:

```bash
modsanity status
```

### `modsanity deploy [--method ...]`
Deploys enabled mods to the active game.

- If `--method` is provided, deployment method is set first (`symlink|hardlink|copy`) then deploy runs.

Usage:

```bash
modsanity deploy
modsanity deploy --method hardlink
```

### `modsanity doctor [--verbose]`
Runs environment diagnostics with checks and remediation hints.

Checks include (current implementation):

- config/database paths
- downloads/staging existence and writability
- detected game counts by platform
- active game executable/data path checks
- Proton prefix and plugins/loadorder target checks
- Proton runtime/command availability
- configured external tool path checks
- dependency checks (`unrar`, `loot`, `dotnet`, `protontricks`)
- Nexus API key presence

Verbose mode additionally prints custom game entries.

Usage:

```bash
modsanity doctor
modsanity doctor --verbose
```

### `modsanity init [OPTIONS]`
Guided setup command.

Behavior:

- optionally prompts for missing values (`--interactive`)
- scans games
- optionally applies downloads/staging path overrides
- optionally registers custom game path
- selects game
- prints next-step commands

Usage:

```bash
modsanity init --interactive
modsanity init --game-id skyrimse --platform steam
modsanity init --game-id skyrimse --platform gog --game-path /path/to/game
modsanity init --game-id skyrimse --platform manual --game-path /path --proton-prefix /path/to/prefix
```

Options:

- `--interactive`
- `--game-id <GAME_ID>`
- `--platform <steam|gog|manual>` (default `steam`)
- `--game-path <PATH>`
- `--downloads-dir <PATH>`
- `--staging-dir <PATH>`
- `--proton-prefix <PATH>`

### `modsanity audit --dry-run`
Runs no-write setup analysis for active game:

- installed/enabled mods/plugins counts
- missing masters
- load-order issues
- conflict summary

Usage:

```bash
modsanity audit --dry-run
```

### `modsanity getting-started`
Prints a practical first-run command sequence.

Usage:

```bash
modsanity getting-started
```

## 4. Game Commands

Group usage:

```bash
modsanity game <COMMAND>
```

### `game list`
Lists detected games and marks active one.

```bash
modsanity game list
```

### `game scan`
Rescans Steam + custom game path entries.

```bash
modsanity game scan
```

### `game select <NAME>`
Selects active game by ID or partial name match.

```bash
modsanity game select skyrimse
modsanity game select "Skyrim Special"
```

### `game info`
Prints active game details (platform, paths, prefix/appdata when present).

```bash
modsanity game info
```

### `game add-path <GAME_ID> <PATH> [--platform ...] [--proton-prefix ...]`
Registers/updates custom install path for Steam/GOG/manual entries.

```bash
modsanity game add-path skyrimse /games/SkyrimSE --platform gog
modsanity game add-path skyrimse /games/SkyrimSE --platform manual --proton-prefix /path/to/compatdata
```

### `game remove-path <GAME_ID> <PATH>`
Removes previously added custom install path.

```bash
modsanity game remove-path skyrimse /games/SkyrimSE
```

## 5. Mod Commands

Group usage:

```bash
modsanity mod <COMMAND>
```

### `mod list`
Lists installed mods for active game.

```bash
modsanity mod list
```

### `mod install <PATH>`
Installs archive (`.zip`, `.7z`, `.rar`) into staging + DB.

Notes:

- If archive requires FOMOD wizard interaction, CLI install fails intentionally and instructs to use TUI.

```bash
modsanity mod install /path/to/mod.7z
```

### `mod enable <NAME>` / `mod disable <NAME>`
Toggles mod enable state. Deployment required to apply to game directory.

```bash
modsanity mod enable "SkyUI"
modsanity mod disable "SkyUI"
```

### `mod remove <NAME>`
Removes installed mod entry/files from staging/DB workflow.
c
```bash
modsanity mod remove "SkyUI"
```

### `mod info <NAME>`
Prints mod metadata (version, enabled state, priority, Nexus ID when present, file count).

```bash
modsanity mod info "SkyUI"
```

### `mod rescan`
Scans staging directory and syncs discovered mods/plugins into DB.

```bash
modsanity mod rescan
```

## 6. Profile Commands

Group usage:

```bash
modsanity profile <COMMAND>
```

### `profile list`
Lists profiles for active game (marks active profile).

```bash
modsanity profile list
```

### `profile create <NAME>`
Creates a profile for active game.

```bash
modsanity profile create "VanillaPlus"
```

### `profile switch <NAME>`
Switches active profile. Deployment required to apply.

```bash
modsanity profile switch "VanillaPlus"
```

### `profile delete <NAME>`
Deletes profile.

```bash
modsanity profile delete "OldProfile"
```

### `profile export <NAME> <PATH>`
Exports profile to file.

```bash
modsanity profile export "VanillaPlus" /tmp/vanillaplus.profile.json
```

### `profile import <PATH>`
Imports profile file for active game.

```bash
modsanity profile import /tmp/vanillaplus.profile.json
```

## 7. Import and Queue Commands

## 7.1 Import Commands

Group usage:

```bash
modsanity import <COMMAND>
```

### `import modlist <PATH> [--auto-approve] [--preview]`
Imports MO2 `modlist.txt` through matching pipeline.

Behavior:

- requires active game + Nexus API key
- performs matching and prints summary
- stores imported modlist in DB (unless `--preview`)
- creates queue batch (unless `--preview`)
- with `--auto-approve`, immediately processes created queue batch

```bash
modsanity import modlist /path/to/modlist.txt
modsanity import modlist /path/to/modlist.txt --preview
modsanity import modlist /path/to/modlist.txt --auto-approve
```

### `import status [BATCH_ID]`
Shows entry-level status for an import batch.

- If `BATCH_ID` omitted, uses latest batch (optionally filtered by active game context).

```bash
modsanity import status
modsanity import status 20260208-abc123
```

### `import apply-enabled <PATH> [--preview]`
Applies MO2 plugin enabled/disabled state onto already-installed mods (migration bridge).

Behavior:

- parses MO2 `modlist.txt`
- resolves plugin filenames to installed mods through DB plugin index
- enables/disables resolved installed mods
- unresolved/ambiguous plugin mappings are reported in summaryc

```bash
modsanity import apply-enabled /path/to/modlist.txt --preview
modsanity import apply-enabled /path/to/modlist.txt
```

## 7.2 Queue Commands

Group usage:

```bash
modsanity queue <COMMAND>
```

### `queue list`
Lists queue batch summaries for active game context (if active game exists).

```bash
modsanity queue list
```

### `queue process [--batch-id <ID>] [--download-only]`
Processes queue batches.

Behavior:

- requires active game + Nexus API key
- with `--batch-id`, processes only that batch
- without `--batch-id`, processes all batches for active game
- `--download-only` skips install step

```bash
modsanity queue process --batch-id 20260208-abc123
modsanity queue process --download-only
```

### `queue retry`
Finds failed entries for active game and retries them by batch.

```bash
modsanity queue retry
```

### `queue clear [BATCH_ID]`
Clears queue data.

- With `BATCH_ID`: clears only that batch.
- Without argument: clears all batches.

```bash
modsanity queue clear 20260208-abc123
modsanity queue clear
```

## 8. Modlist Commands

Group usage:

```bash
modsanity modlist <COMMAND>
```

### `modlist save <PATH> [--format native|mo2]`
Exports current active-game state and stores modlist in DB.

Formats:

- `native` (JSON, default)
- `mo2` (text)

```bash
modsanity modlist save /tmp/list.json --format native
modsanity modlist save /tmp/modlist.txt --format mo2
```

### `modlist load <PATH> [--auto-approve] [--preview]`
Loads a modlist file. Format is auto-detected:

- native format -> native load path
- MO2 format -> delegated to `import modlist`

Behavior:

- validates active game for native files
- stores imported/loaded modlist in DB (unless `--preview`)
- may create queue entries (unless `--preview`)
- `--auto-approve` immediately processes resulting queue batch

```bash
modsanity modlist load /tmp/list.json
modsanity modlist load /tmp/list.json --preview
modsanity modlist load /tmp/modlist.txt --auto-approve
```

## 9. Nexus Commands

Group usage:

```bash
modsanity nexus <COMMAND>
```

### `nexus populate --game <DOMAIN> [--reset] [--per-page N] [--max-pages N]`
Populates local SQLite catalog from Nexus REST API.

- requires Nexus API key
- supports resume/checkpoint behavior by default
- `--reset` starts from beginning

```bash
modsanity nexus populate --game skyrimspecialedition
modsanity nexus populate --game skyrimspecialedition --reset --per-page 100 --max-pages 10
```

### `nexus status --game <DOMAIN>`
Shows sync status/checkpoint and record count for a game domain.

```bash
modsanity nexus status --game skyrimspecialedition
```

## 10. Deployment Commands

Group usage:

```bash
modsanity deployment <COMMAND>
```

### `deployment show`
Prints deployment settings and resolved downloads/staging paths.

```bash
modsanity deployment show
```

### `deployment set-method <symlink|hardlink|copy>`
Sets default deployment method.

```bash
modsanity deployment set-method symlink
modsanity deployment set-method hardlink
modsanity deployment set-method copy
```

### `deployment set-downloads-dir <PATH>` / `deployment clear-downloads-dir`
Sets or clears downloads directory override.

```bash
modsanity deployment set-downloads-dir /mnt/storage/Downloads
modsanity deployment clear-downloads-dir
```

### `deployment set-staging-dir <PATH>` / `deployment clear-staging-dir`
Sets or clears staging directory override.

```bash
modsanity deployment set-staging-dir /mnt/storage/ModSanity/Staging
modsanity deployment clear-staging-dir
```

### `deployment migrate-staging <FROM> <TO> [--dry-run]`
Performs a safe staging migration by recursively copying files from source staging root to destination.

Behavior:

- computes migration plan (directories/files/skips)
- skips destination files that already exist
- with `--dry-run`, prints plan only
- on apply, updates staging override to destination

```bash
modsanity deployment migrate-staging /old/staging /new/staging --dry-run
modsanity deployment migrate-staging /old/staging /new/staging
```

## 11. External Tool Commands (`tool`)

Group usage:

```bash
modsanity tool <COMMAND>
```

### `tool show`
Shows:

- current runtime mode (`proton_runtime` selection or custom command mode)
- fallback command
- resolved Proton launcher path
- detected runtime list
- configured tool executable paths

```bash
modsanity tool show
```

### `tool list-proton`
Detects and lists Steam-managed Proton runtimes (from Steam libraries and `compatibilitytools.d`).

```bash
modsanity tool list-proton
```

### `tool use-proton <RUNTIME_ID|auto>`
Selects a detected Proton runtime by ID/name/path, or `auto`.

```bash
modsanity tool use-proton auto
modsanity tool use-proton steam:proton_experimental
```

### `tool clear-proton-runtime`
Clears runtime selection and returns to custom command/path mode.

```bash
modsanity tool clear-proton-runtime
```

### `tool set-proton <PATH_OR_COMMAND>`
Sets custom Proton launcher command/path and clears selected runtime mode.

```bash
modsanity tool set-proton proton
modsanity tool set-proton /path/to/proton
```

### `tool set-path <TOOL> <PATH>` / `tool clear-path <TOOL>`
Sets/clears executable path for supported tools.

Tool IDs:

- `xedit`
- `ssedit` / `sseedit`
- `fnis`
- `nemesis`
- `symphony`
- `bodyslide`
- `outfitstudio`

```bash
modsanity tool set-path xedit "/path/to/SSEEdit.exe"
modsanity tool clear-path xedit
```

### `tool set-runtime <TOOL> <proton|native>` / `tool clear-runtime <TOOL>`
Sets/clears per-tool runtime launch mode.

- `proton` uses Proton launcher/runtime resolution
- `native` executes tool path directly on host
- clearing runtime resets tool to default `proton` mode

```bash
modsanity tool set-runtime xedit proton
modsanity tool set-runtime symphony native
modsanity tool clear-runtime symphony
```

### `tool run <TOOL> [ARGS]...`
Launches configured tool via Proton for active game.

Behavior:

- requires active game and detected/configured Proton prefix
- injects Proton/Wine environment (`STEAM_COMPAT_DATA_PATH`, `WINEPREFIX`)
- forwards all extra args to tool executable

```bash
modsanity tool run xedit
modsanity tool run xedit -IKnowWhatImDoing -quickautoclean
```

## 12. Practical End-to-End Examples

## 12.1 Fresh setup (Steam)

```bash
modsanity init --interactive
modsanity game list
modsanity game select skyrimse
modsanity doctor --verbose
```

## 12.2 Set custom storage paths

```bash
modsanity deployment set-downloads-dir /data/downloads
modsanity deployment set-staging-dir /data/modsanity/staging
modsanity deployment show
```

## 12.3 Install + deploy

```bash
modsanity mod install /data/downloads/SkyUI_5_2SE.7z
modsanity mod enable SkyUI
modsanity deploy
```

## 12.4 Import MO2 modlist in safe mode

```bash
modsanity import modlist /path/to/modlist.txt --preview
modsanity import modlist /path/to/modlist.txt
modsanity import status
modsanity queue process
```

## 12.5 Configure Proton runtime and run tool

```bash
modsanity tool list-proton
modsanity tool use-proton auto
modsanity tool set-path xedit "/path/to/SSEEdit.exe"
modsanity tool run xedit
```

## 13. Troubleshooting Quick Notes

- “No game selected”: run `modsanity game list` then `modsanity game select <id>`.
- Nexus errors: set `nexus_api_key` in config and verify with `modsanity doctor --verbose`.
- Tool launch failures: verify runtime and tool paths with `modsanity tool show`.
- Queue/import issues: inspect `modsanity import status [batch]` and `modsanity queue list`.

