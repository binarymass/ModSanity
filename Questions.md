ModSanity Comment-Derived Roadmap (Verification-First)
How Codex should respond (required format)

For every bullet below, Codex should output:

Status: Implemented | Partial | Not Implemented

Evidence: file paths + symbols (functions/structs/modules/commands)

How to reproduce: exact CLI command(s)

Gaps: what still fails / missing UX / missing docs / missing edge case

Tests: test name(s) or “none”

Phase 0 — Baseline Inventory & Truth Pass

Goal: establish what exists before implementing anything.

0.1 CLI Capability Map

Q: Is there a single command that prints all supported operations and their flags (e.g., modsanity --help plus subcommand help)?

Q: Is there a modsanity doctor or modsanity diagnose command that audits system state (Steam/GOG detection, game paths, tool runtimes, permissions, proton availability)?

Q: Is there a “first run” state persisted anywhere (config + initialization marker)?

0.2 Config Surface Area

Q: Is there a single .toml (or config) source-of-truth with documented keys?

Q: Are config keys validated, with defaults, and clear error messages on invalid paths?

Deliverable: docs/verification/phase0_inventory.md containing:

command list

config keys

folder layout

tool detection logic list

Phase 1 — Installation & Onboarding (Linux Newbie Friction)

Driven by: “ran install.sh, don’t know what to do next”, scan not working.

1.1 Guided First-Run

Q: Does modsanity init exist?

Q: Does it guide the user through:

selecting game source (Steam, GOG, Manual)

detecting game install path

confirming writable mod staging paths

saving config

Q: Does it print “next commands to run” at the end?

1.2 Game Scan Reliability

Q: Does modsanity scan exist and succeed without requiring the user to memorize bash lines?

Q: If scan fails, does it provide actionable next steps (paths searched, permissions, expected directories)?

1.3 Documentation for Day-1 Users

Q: Is there a minimal quickstart that matches current CLI exactly?

Q: Are install + first run instructions validated against the actual code paths?

Deliverable: docs/quickstart.md + a smoke test script (optional).

Phase 2 — Path Flexibility & Non-Data Installs (SKSE-class)

Driven by: move mods folder from ~/.local, and mods that install outside Data/.

2.1 Mods Folder Relocation

Q: Is the mods folder location configurable via .toml (e.g., mods_dir)?

Q: Is there a CLI override flag (e.g., --mods-dir)?

Q: Does the tool migrate/handle existing content safely if the path changes?

2.2 Non-Data Deployment Classes

Q: Is there a concept of “install target” (Data dir vs game root vs other)?

Q: Are SKSE-like mods supported as first-class installs (not “manual required”)?

Q: Are there rules/manifest logic to place files correctly?

Deliverable: Install target model documented (even if minimal):

data/

game root

specific subpaths (SKSE plugins, ENB, etc.)

Phase 3 — Toolchain Support (The Big One)

Driven by repeated asks: xEdit, Bodyslide, Synthesis, Nemesis/Pandora, Skypatcher.

3A — Tool Runner Architecture (Foundation)

Q: Is there a unified tool runner subsystem (registry + launcher)?

Q: Can tools be defined declaratively (config) + invoked consistently (CLI)?

Q: Is runtime selection supported (Native vs Proton/Wine)?

Q: Are environment variables + working directory + load order context injected?

3B — Specific Tool Integrations

Each tool below must answer: “can I run it from ModSanity and it sees the correct mod environment?”

xEdit / SSEEdit

Q: Does modsanity tools run xedit exist (or equivalent)?

Q: Does it work under Proton/Wine with required runtimes present?

Q: Are xEdit arguments configurable?

Bodyslide

Q: Is Bodyslide supported with correct output paths?

Q: Does it respect profiles / mod staging?

Synthesis

Q: Is Synthesis supported either natively (dotnet) or via Proton?

Q: Are the “root cert / dotnet restore” proton issues handled via helper script or doctor flow?

Q: Does ModSanity provide the “mods installed environment” Synthesis expects?

Nemesis / Pandora / animation tools

Q: Is there an integration path to run these tools with correct mod list + outputs?

Q: Are outputs placed correctly and tracked?

Skypatcher / patchers

Q: Is there a general “patcher” framework, or is each patcher bespoke?

Q: Are patch outputs tracked and reversible?

Deliverables:

modsanity tools list

modsanity tools run <tool>

modsanity doctor checks: proton prefix, dotnet, wine deps, certificates

Phase 4 — MO2 Interoperability & Migration

Driven by: import load order, handling VFS, migration blockers.

4.1 Import Load Order / Profile

Q: Can ModSanity import:

mod list (enabled/disabled)

plugin load order

profile-specific settings

Q: Does it support a read-only “preview import” mode?

4.2 VFS Explanation & Bridging

Q: Does ModSanity clearly define its model vs MO2 VFS?

Q: Is there any bridging mode to reduce migration pain (even if limited)?

Q: Are there docs that specifically answer: “How do you handle VFS?”

Deliverable:

docs/migration/mo2.md including “what transfers” and “what doesn’t”.

Phase 5 — Platform Coverage: GOG & Non-Steam

Driven by multiple users asking about GOG.

5.1 GOG Detection & Pathing

Q: Is GOG explicitly supported (not just “might work”)?

Q: Can the user select platform in config and have consistent folder/compatdata handling?

5.2 Manual Install Support (Fallback)

Q: Can a user provide a game path and have everything work without Steam metadata?

Deliverable:

modsanity init supports Steam/GOG/Manual

modsanity doctor reports platform status

Phase 6 — Packaging & Distribution (Arch / AUR)

Driven by repeated Arch+AUR asks.

6.1 AUR Packaging Readiness

Q: Is there an AUR-ready PKGBUILD (or release artifacts) maintained?

Q: Are versioning and releases consistent and reproducible?

Deliverable:

/packaging/arch/PKGBUILD (or community docs)

Phase 7 — UX Accessibility without Abandoning TUI

Driven by “terminal is a big no”, migraine/contrast concerns, while others love TUI.

7.1 Theme / Color Handling

Q: Are colors strictly terminal-driven (no hardcoding), and documented?

Q: Is there an optional “minimal color” mode?

7.2 Windows Refugee Mitigation (Without GUI)

Q: Does the tool provide discoverability:

modsanity help getting-started

“suggested next command” hints

Q: Are error messages “human first”?

Deliverable:

docs/ui/accessibility.md

Phase 8 — Scale Confidence: Huge Mod Lists

Driven by fear of testing on main list, “huge and confuse”.

8.1 Dry-Run / Audit Mode

Q: Is there a no-write analysis mode (dry-run)?

Q: Does it summarize:

number of mods/plugins found

missing masters / dependency issues

conflicts detected (even basic)

8.2 Safe Experiment Support

Q: Is there support for “profiles” (separate mod lists under one profile)?

(You already mentioned this is being worked on / needed.)

Deliverable:

modsanity audit --dry-run

profile/list isolation validation

Output Format for Codex 5.3 (paste this directly)

Use this exact structure so you can diff results between versions:

PHASE X.Y — <Title>
Item: <exact question>
Status: Implemented | Partial | Not Implemented
Evidence:
- <file_path>:<line or symbol>
- <module::type::function>
Reproduce:
- <command>
Notes:
- <what works>
- <what fails>
Gaps:
- <missing behavior>
Tests:
- <test name(s)> | none

Minimal “What to check first” order (fastest truth pass)

Phase 0 (inventory)

Phase 2 (paths + SKSE class)

Phase 3 (tool runner foundation)

Phase 4 (MO2 import + VFS answer)

Phase 5 (GOG)
