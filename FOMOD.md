ModSanity — Full FOMOD Support Plan (Markdown)
Goal

Implement full, MO2/Vortex-grade FOMOD support in a Linux-native, deterministic, keyboard-first workflow—without letting installers “mystery-meat” the user’s mod state.

0) Definition of “Full FOMOD Support”

A mod is considered fully supported if ModSanity can:

Detect and parse FOMOD metadata (fomod/info.xml, fomod/ModuleConfig.xml).

Render installer UI from ModuleConfig.xml:

Pages / Steps

Groups

Options

Required / Recommended / Optional types

Visibility conditions

Conditional flags

Resolve the installer into an explicit install plan:

Selected options

Files/folders to install

Optional file mappings/renames (where applicable)

Execute installation transactionally:

Preview

Apply

Rollback on failure

Persist decisions so installer can be re-run for a mod or per-profile.

1) High-Level Architecture
1.1 New Subsystem: fomod

Create a dedicated module with strict boundaries:

fomod::detect — find FOMOD presence and collect paths

fomod::parse — parse info.xml + ModuleConfig.xml

fomod::model — typed representation of installer structure

fomod::eval — condition evaluation engine

fomod::plan — compile selections into install actions

fomod::execute — transactional apply/rollback

fomod::ui — TUI pages and selection UX

Key invariant: FOMOD produces an InstallPlan; only the installer executor touches the filesystem.

2) Phase 1 — Detection + Metadata + Safe Extraction
2.1 Detect FOMOD

If archive contains fomod/ModuleConfig.xml → “FOMOD installer available”

If only fomod/info.xml → show metadata (still treat as FOMOD-capable if ModuleConfig.xml missing)

2.2 Extraction Strategy

Extract archive to staging location:

staging/{mod_id}/{hash}/...

Staging is read-only for parsing; execution copies out selected files.

2.3 Metadata Display

Show info.xml fields:

Name, version, author, website, description (if present)

Useful even when config is minimal.

Deliverable:

A mod can be flagged as “FOMOD” and metadata is visible.

3) Phase 2 — XML Parser + Installer Model
3.1 XML Parsing

Implement robust parsing for:

ModuleConfig.xml with error reporting:

Unknown tags -> warning

Invalid schema -> fail with actionable message

3.2 Internal Model (Core Types)

Model objects you’ll need:

FomodInstaller

install_steps: Vec<InstallStep>

conditions: ConditionTable (if present)

InstallStep

name

groups: Vec<Group>

Group

name

group_type (SelectExactlyOne / SelectAtMostOne / SelectAtLeastOne / SelectAll / Optional)

options: Vec<OptionItem>

OptionItem

name

description

image (optional)

flags: Vec<String>

type (Required/Recommended/Optional)

visible_if: ConditionExpr

install_rules: Vec<InstallRule>

3.3 Condition Language

Implement a small boolean expression evaluator supporting:

AND / OR / NOT

Flag presence checks

Comparisons if spec uses them (some FOMODs do)

“Dependency flags” that appear only after selecting options

Deliverable:

You can parse and represent installers deterministically with complete diagnostics.

4) Phase 3 — Interactive TUI Installer (Keyboard-First)
4.1 UI Requirements

Dedicated “Run FOMOD Installer” action per mod

Pages:

Overview (metadata)

Step navigation

Group option selection

Summary (selected flags + resulting files)

Confirm / Apply

4.2 Navigation

j/k: move

space: toggle option

enter: continue

tab: next group

b: back

s: save/apply

esc: cancel (no state change)

4.3 Live Validation

If group constraints violated:

Mark group as incomplete

Block “Apply” until satisfied

Provide a clear message:

“Select exactly one option in group X”

“Select at least one option in group Y”

Deliverable:

A user can complete complex FOMOD installers without leaving ModSanity.

5) Phase 4 — Plan Compiler (Selections → InstallPlan)
5.1 InstallPlan Structure

The compiled plan must be explicit and replayable:

InstallPlan

mod_id

profile_id (optional if profile-specific)

selected_options: Vec<OptionRef>

flags_set: HashSet<String>

file_ops: Vec<FileOp>

conflict_preview: Vec<ConflictItem>

source_staging_path

target_mod_path

5.2 File Operations

Support operations common across FOMODs:

Copy directory

Copy file

Conditional copy

Destination mapping (if installer specifies)

5.3 Conflict Preview

Before applying:

Compare planned outputs vs existing mod files in profile

Show list of overwrites/new files

Let user proceed or cancel

Deliverable:

FOMOD becomes transparent: user sees exactly what will happen.

6) Phase 5 — Transactional Executor + Rollback
6.1 Transaction Design

Executor must ensure:

If anything fails, the mod folder returns to pre-state.

Implementation strategy:

Apply into target_mod_path/.tmp_install_{txn_id}

If success: atomic rename swap

If fail: delete temp folder

6.2 Rollback Safety

Always keep a backup of previous folder (or use rename swap):

mod_path -> mod_path.bak_{txn_id}

tmp -> mod_path

cleanup backups after success

6.3 Logging

Emit structured log entries:

Install start / end

Selected flags/options

Files installed

Conflicts resolved

Errors + rollback result

Deliverable:

Install is safe, debuggable, and trustworthy.

7) Phase 6 — Persistence, Re-run, and Profile Integration
7.1 Persist Choices

Store installer run output:

mod_id

profile_id (if profile-specific)

selected options

flags set

timestamp

installer hash (so changes invalidate old choices)

7.2 Re-run Support

If user re-runs installer:

Preselect last used options

If installer changed:

show “installer updated” and reset selections

optionally offer a migration attempt if options unchanged

7.3 Profile-specific Installation

Support two modes:

Global install (same installed files for all profiles)

Per-profile install (installer outputs differ by profile)

Deliverable:

FOMOD is first-class in your profile workflow, not a one-off.

8) Phase 7 — Advanced Compatibility (Edge Cases)
8.1 Nested Data Roots

Some archives have:

00 Core/Data/...

Data/...

or Skyrim Special Edition/Data/...

Add root detection:

Identify “Data-like” root

Normalize to staging root for copy ops

8.2 “No FOMOD but similar behavior”

Some mods ship scripted logic without true FOMOD.
Provide fallback:

“Manual file picker / variant installer”

This complements FOMOD without pretending it’s FOMOD.

8.3 Installer Images

Optional but useful:

Render preview images if present (TUI: show file path; optionally open external viewer)

Deliverable:

Handle the wild mods without corrupting your core design.

9) Testing Plan (Must-Have)
9.1 Unit Tests

XML parsing (valid/invalid)

Condition evaluation

Group constraint enforcement

Plan compilation results

9.2 Golden Installers

Maintain a suite of known FOMOD examples:

simple single-step

multi-step with flags

complex conditionals

conflicting outputs

9.3 Transaction Tests

Simulated failure mid-install triggers rollback

Backup cleanup on success

Atomic swap correctness

10) Milestones Checklist
Milestone A — “Recognize FOMOD”

Detect FOMOD + show metadata

Milestone B — “Parse FOMOD”

Installer model created + diagnostics

Milestone C — “Run FOMOD”

TUI installer works for multi-step configs

Milestone D — “Install Safely”

Transactional apply + rollback

Milestone E — “Re-run & Profiles”

Save selections, re-run, per-profile behavior supported

Milestone F — “Compatibility”

Root detection, edge-case support, robust error handling

11) Non-Negotiable Invariants

No silent installs. User can always preview what files will be deployed.

No partial state. Either install completes or state rolls back.

Deterministic outputs. Same selections → same install plan → same results.

FOMOD is a subsystem. It cannot leak complexity into the rest of the codebase.
