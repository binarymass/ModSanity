//! Plugin header parsing (ESP/ESM/ESL)
//!
//! Parses the TES4/TES5 record header to extract plugin metadata

use anyhow::{Context, Result};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Parsed plugin header information
#[derive(Debug, Clone, Default)]
pub struct PluginHeader {
    /// Plugin type signature (TES4, TES5, etc.)
    pub signature: String,

    /// Record flags
    pub flags: u32,

    /// Is this a master file (ESM flag set)
    pub is_master: bool,

    /// Is this a light plugin (ESL flag set)
    pub is_light: bool,

    /// Master file dependencies
    pub masters: Vec<String>,

    /// Plugin description (SNAM)
    pub description: Option<String>,

    /// Plugin author (CNAM)
    pub author: Option<String>,
}

impl PluginHeader {
    /// ESM flag bit
    const FLAG_MASTER: u32 = 0x00000001;

    /// ESL flag bit (light plugin)
    const FLAG_LIGHT: u32 = 0x00000200;
}

/// Parse plugin header from file
pub fn parse_plugin_header(path: &Path) -> Result<PluginHeader> {
    let mut file = std::fs::File::open(path).context("Failed to open plugin")?;
    let mut header = PluginHeader::default();

    // Read record type (4 bytes)
    let mut sig = [0u8; 4];
    file.read_exact(&mut sig)?;
    header.signature = String::from_utf8_lossy(&sig).to_string();

    // Verify it's a valid plugin
    if !["TES4", "TES5"].contains(&header.signature.as_str()) {
        anyhow::bail!("Invalid plugin signature: {}", header.signature);
    }

    // Read data size (4 bytes)
    let mut size_bytes = [0u8; 4];
    file.read_exact(&mut size_bytes)?;
    let data_size = u32::from_le_bytes(size_bytes);

    // Read flags (4 bytes)
    let mut flags_bytes = [0u8; 4];
    file.read_exact(&mut flags_bytes)?;
    header.flags = u32::from_le_bytes(flags_bytes);

    header.is_master = (header.flags & PluginHeader::FLAG_MASTER) != 0;
    header.is_light = (header.flags & PluginHeader::FLAG_LIGHT) != 0;

    // Skip form ID and version info (8 bytes)
    file.seek(SeekFrom::Current(8))?;

    // Read subrecords within the header record
    let header_end = file.stream_position()? + data_size as u64;

    while file.stream_position()? < header_end {
        // Read subrecord type (4 bytes)
        let mut sub_type = [0u8; 4];
        if file.read_exact(&mut sub_type).is_err() {
            break;
        }
        let sub_type_str = String::from_utf8_lossy(&sub_type);

        // Read subrecord size (2 bytes)
        let mut sub_size_bytes = [0u8; 2];
        file.read_exact(&mut sub_size_bytes)?;
        let sub_size = u16::from_le_bytes(sub_size_bytes) as usize;

        match sub_type_str.as_ref() {
            "MAST" => {
                // Master file dependency
                let mut data = vec![0u8; sub_size];
                file.read_exact(&mut data)?;
                // Remove null terminator
                if let Some(pos) = data.iter().position(|&b| b == 0) {
                    data.truncate(pos);
                }
                let master = String::from_utf8_lossy(&data).to_string();
                header.masters.push(master);

                // Skip DATA subrecord that follows MAST
                let mut check = [0u8; 4];
                if file.read_exact(&mut check).is_ok() && &check == b"DATA" {
                    let mut data_size = [0u8; 2];
                    file.read_exact(&mut data_size)?;
                    let skip = u16::from_le_bytes(data_size);
                    file.seek(SeekFrom::Current(skip as i64))?;
                } else {
                    file.seek(SeekFrom::Current(-4))?;
                }
            }
            "SNAM" => {
                // Description
                let mut data = vec![0u8; sub_size];
                file.read_exact(&mut data)?;
                if let Some(pos) = data.iter().position(|&b| b == 0) {
                    data.truncate(pos);
                }
                header.description = Some(String::from_utf8_lossy(&data).to_string());
            }
            "CNAM" => {
                // Author
                let mut data = vec![0u8; sub_size];
                file.read_exact(&mut data)?;
                if let Some(pos) = data.iter().position(|&b| b == 0) {
                    data.truncate(pos);
                }
                header.author = Some(String::from_utf8_lossy(&data).to_string());
            }
            _ => {
                // Skip unknown subrecords
                file.seek(SeekFrom::Current(sub_size as i64))?;
            }
        }
    }

    Ok(header)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flag_parsing() {
        let header = PluginHeader {
            flags: PluginHeader::FLAG_MASTER | PluginHeader::FLAG_LIGHT,
            ..Default::default()
        };

        assert!((header.flags & PluginHeader::FLAG_MASTER) != 0);
        assert!((header.flags & PluginHeader::FLAG_LIGHT) != 0);
    }
}
