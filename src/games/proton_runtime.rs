//! Proton runtime detection (Steam-managed installs and compatibility tools)

use regex_lite::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Detected Proton runtime candidate.
#[derive(Debug, Clone)]
pub struct ProtonRuntime {
    /// Stable ID used for config selection.
    pub id: String,
    /// Human-friendly name (directory name).
    pub name: String,
    /// Path to executable `proton` launcher script.
    pub proton_path: PathBuf,
    /// Discovery source path context.
    pub source: String,
}

/// Detect available Proton runtimes from Steam-managed install locations.
pub fn detect_proton_runtimes() -> Vec<ProtonRuntime> {
    let mut steam_roots = candidate_steam_roots();
    let mut library_roots = Vec::new();
    for root in &steam_roots {
        library_roots.extend(read_libraryfolders(root));
    }
    steam_roots.extend(library_roots);

    let mut seen_runtime_paths = HashSet::new();
    let mut runtimes = Vec::new();
    for root in steam_roots {
        scan_runtime_dir(
            &root.join("steamapps/common"),
            "steamapps/common",
            &mut seen_runtime_paths,
            &mut runtimes,
        );
        scan_runtime_dir(
            &root.join("compatibilitytools.d"),
            "compatibilitytools.d",
            &mut seen_runtime_paths,
            &mut runtimes,
        );
    }

    runtimes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    runtimes
}

fn candidate_steam_roots() -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    if let Ok(steam_dir) = std::env::var("STEAM_DIR") {
        push_existing_path(&mut out, &mut seen, Path::new(&steam_dir));
    }

    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        push_existing_path(&mut out, &mut seen, &home.join(".steam/root"));
        push_existing_path(&mut out, &mut seen, &home.join(".local/share/Steam"));
        push_existing_path(
            &mut out,
            &mut seen,
            &home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        );
        push_existing_path(
            &mut out,
            &mut seen,
            &home.join("snap/steam/common/.local/share/Steam"),
        );
    }

    out
}

fn push_existing_path(out: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, candidate: &Path) {
    if !candidate.exists() {
        return;
    }
    let normalized = std::fs::canonicalize(candidate).unwrap_or_else(|_| candidate.to_path_buf());
    if seen.insert(normalized.clone()) {
        out.push(normalized);
    }
}

fn read_libraryfolders(steam_root: &Path) -> Vec<PathBuf> {
    let path = steam_root.join("steamapps/libraryfolders.vdf");
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };

    // Supports both modern ("path" "...") and older ("1" "...") formats.
    let path_re = Regex::new(r#""path"\s*"([^"]+)""#).expect("valid regex");
    let legacy_re = Regex::new(r#""\d+"\s*"([^"]+)""#).expect("valid regex");

    let mut seen = HashSet::new();
    let mut libraries = Vec::new();

    for cap in path_re.captures_iter(&content) {
        if let Some(m) = cap.get(1) {
            let p = normalize_vdf_path(m.as_str());
            if seen.insert(p.clone()) {
                libraries.push(p);
            }
        }
    }

    for cap in legacy_re.captures_iter(&content) {
        if let Some(m) = cap.get(1) {
            let candidate = normalize_vdf_path(m.as_str());
            if candidate.join("steamapps").exists() && seen.insert(candidate.clone()) {
                libraries.push(candidate);
            }
        }
    }

    libraries
}

fn normalize_vdf_path(raw: &str) -> PathBuf {
    let unescaped = raw.replace("\\\\", "\\");
    PathBuf::from(unescaped)
}

fn scan_runtime_dir(
    dir: &Path,
    source: &str,
    seen_runtime_paths: &mut HashSet<PathBuf>,
    out: &mut Vec<ProtonRuntime>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let proton_path = path.join("proton");
        if !proton_path.is_file() {
            continue;
        }

        let normalized = std::fs::canonicalize(&proton_path).unwrap_or(proton_path);
        if !seen_runtime_paths.insert(normalized.clone()) {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        out.push(ProtonRuntime {
            id: format!("steam:{}", slugify(&name)),
            name,
            proton_path: normalized,
            source: source.to_string(),
        });
    }
}

fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if ch == ' ' || ch == '-' || ch == '_' || ch == '.' {
            out.push('_');
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_string()
}
