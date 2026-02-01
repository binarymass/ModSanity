//! Archive extraction utilities (zip, 7z, rar)

use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;

/// Progress callback for extraction
/// Parameters: (current_file, processed_count, total_count)
pub type ProgressCallback = Arc<dyn Fn(String, usize, usize) + Send + Sync>;

/// Supported archive formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    SevenZip,
    Rar,
    Unknown,
}

impl ArchiveFormat {
    /// Detect format from file extension
    pub fn from_path(path: &Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        match ext.as_str() {
            "zip" => Self::Zip,
            "7z" => Self::SevenZip,
            "rar" => Self::Rar,
            _ => Self::Unknown,
        }
    }
}

/// Extract an archive to the destination directory
pub async fn extract_archive(
    archive: &Path,
    dest: &Path,
    progress_callback: Option<ProgressCallback>,
) -> Result<()> {
    let format = ArchiveFormat::from_path(archive);

    // Ensure destination exists
    tokio::fs::create_dir_all(dest).await?;

    match format {
        ArchiveFormat::Zip => extract_zip(archive, dest, progress_callback),
        ArchiveFormat::SevenZip => extract_7z(archive, dest, progress_callback),
        ArchiveFormat::Rar => extract_rar(archive, dest, progress_callback),
        ArchiveFormat::Unknown => {
            // Try to detect from magic bytes
            let bytes = std::fs::read(archive)?;
            if bytes.starts_with(&[0x50, 0x4B]) {
                extract_zip(archive, dest, progress_callback)
            } else if bytes.starts_with(&[0x37, 0x7A, 0xBC, 0xAF]) {
                extract_7z(archive, dest, progress_callback)
            } else if bytes.starts_with(&[0x52, 0x61, 0x72, 0x21]) {
                extract_rar(archive, dest, progress_callback)
            } else {
                anyhow::bail!("Unknown archive format")
            }
        }
    }
}

/// Extract a ZIP archive
fn extract_zip(archive: &Path, dest: &Path, progress_callback: Option<ProgressCallback>) -> Result<()> {
    let file = std::fs::File::open(archive).context("Failed to open archive")?;
    let mut zip = zip::ZipArchive::new(file).context("Failed to read ZIP archive")?;

    let total = zip.len();

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let entry_name = entry.name().to_string();
        let outpath = dest.join(sanitize_path(&entry_name));

        // Report progress
        if let Some(ref cb) = progress_callback {
            cb(entry_name.clone(), i + 1, total);
        }

        if entry.is_dir() {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut entry, &mut outfile)?;

            // Set permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = entry.unix_mode() {
                    std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode))?;
                }
            }
        }
    }

    Ok(())
}

/// Extract a 7z archive
fn extract_7z(archive: &Path, dest: &Path, progress_callback: Option<ProgressCallback>) -> Result<()> {
    // Note: sevenz_rust doesn't support progress callbacks yet
    // We'll just report at start and end
    if let Some(ref cb) = progress_callback {
        cb("Extracting 7z archive...".to_string(), 0, 100);
    }

    sevenz_rust::decompress_file(archive, dest).context("Failed to extract 7z archive")?;

    if let Some(ref cb) = progress_callback {
        cb("Complete".to_string(), 100, 100);
    }

    Ok(())
}

/// Extract a RAR archive
fn extract_rar(archive: &Path, dest: &Path, progress_callback: Option<ProgressCallback>) -> Result<()> {
    // Note: unrar command-line tool doesn't provide easy progress tracking
    // We'll just report at start and end
    if let Some(ref cb) = progress_callback {
        cb("Extracting RAR archive...".to_string(), 0, 100);
    }

    // Try using system unrar first (more reliable)
    let output = std::process::Command::new("unrar")
        .args(["x", "-o+", "-y"])
        .arg(archive)
        .arg(dest)
        .output();

    let result = match output {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("unrar failed: {}", stderr)
        }
        Err(_) => {
            // unrar not available, try alternative
            anyhow::bail!(
                "RAR extraction requires 'unrar' to be installed.\n\
                 Install it with: sudo apt install unrar (Debian/Ubuntu)\n\
                                  sudo pacman -S unrar (Arch)"
            )
        }
    };

    if result.is_ok() {
        if let Some(ref cb) = progress_callback {
            cb("Complete".to_string(), 100, 100);
        }
    }

    result
}

/// Sanitize path to prevent directory traversal
fn sanitize_path(path: &str) -> String {
    path.replace('\\', "/")
        .split('/')
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_detection() {
        assert_eq!(
            ArchiveFormat::from_path(Path::new("mod.zip")),
            ArchiveFormat::Zip
        );
        assert_eq!(
            ArchiveFormat::from_path(Path::new("mod.7z")),
            ArchiveFormat::SevenZip
        );
        assert_eq!(
            ArchiveFormat::from_path(Path::new("mod.rar")),
            ArchiveFormat::Rar
        );
        assert_eq!(
            ArchiveFormat::from_path(Path::new("mod.ZIP")),
            ArchiveFormat::Zip
        );
    }

    #[test]
    fn test_sanitize_path() {
        assert_eq!(sanitize_path("foo/bar/baz.esp"), "foo/bar/baz.esp");
        assert_eq!(sanitize_path("foo\\bar\\baz.esp"), "foo/bar/baz.esp");
        assert_eq!(sanitize_path("../../../etc/passwd"), "etc/passwd");
        assert_eq!(sanitize_path("./foo/./bar"), "foo/bar");
    }
}
