//! Encode/decode `MyCachedServerItem_MyServerData` to/from Steam server rules.
//!
//! Mirrors the C# methods:
//! - `Sandbox.Game.MyCachedServerItem.SendSettingsToSteam` (encode)
//! - `Sandbox.Game.MyCachedServerItem.DeserializeSettings` (decode)
//!
//! Rule keys:
//! - `"sc"` — the total compressed byte count (decimal string)
//! - `"sc0"`, `"sc1"`, … — base64-encoded 93-byte chunks

use base64::prelude::*;
use std::collections::HashMap;
use std::fmt;

use space_engineers_compat::compression;
use space_engineers_sys::types::MyCachedServerItem_MyServerData;

/// Maximum bytes per rule chunk (matches C# `RULE_LENGTH = 93`).
const RULE_CHUNK_SIZE: usize = 93;

/// Rule key prefix used by Space Engineers.
const RULE_KEY: &str = "sc";

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during server-data rule encoding/decoding.
#[derive(Debug)]
pub enum ServerDataError {
    /// The `"sc"` key is missing from the rules map.
    MissingSizeKey,
    /// The `"sc"` value could not be parsed as a byte count.
    InvalidSizeValue(std::num::ParseIntError),
    /// A required chunk key (`"sc0"`, `"sc1"`, …) is missing.
    MissingChunkKey(String),
    /// Base64 decoding of a chunk failed.
    Base64(base64::DecodeError),
    /// GZip decompression failed.
    Decompression(std::io::Error),
    /// Protobuf decoding of the decompressed payload failed.
    Decode(proto_rs::DecodeError),
}

impl fmt::Display for ServerDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSizeKey => write!(f, "missing 'sc' key in server rules"),
            Self::InvalidSizeValue(e) => write!(f, "invalid 'sc' size value: {e}"),
            Self::MissingChunkKey(k) => write!(f, "missing chunk key '{k}' in server rules"),
            Self::Base64(e) => write!(f, "base64 decode error: {e}"),
            Self::Decompression(e) => write!(f, "decompression error: {e}"),
            Self::Decode(e) => write!(f, "protobuf decode error: {e}"),
        }
    }
}

impl std::error::Error for ServerDataError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidSizeValue(e) => Some(e),
            Self::Base64(e) => Some(e),
            Self::Decompression(e) => Some(e),
            Self::Decode(e) => Some(e),
            _ => None,
        }
    }
}

impl From<base64::DecodeError> for ServerDataError {
    fn from(e: base64::DecodeError) -> Self {
        Self::Base64(e)
    }
}

impl From<std::io::Error> for ServerDataError {
    fn from(e: std::io::Error) -> Self {
        Self::Decompression(e)
    }
}

impl From<proto_rs::DecodeError> for ServerDataError {
    fn from(e: proto_rs::DecodeError) -> Self {
        Self::Decode(e)
    }
}

// ---------------------------------------------------------------------------
// Encode
// ---------------------------------------------------------------------------

/// Encode server data into a Steam rules map.
///
/// Returns a `HashMap` with keys `"sc"`, `"sc0"`, `"sc1"`, … that can be set
/// as Steam Game Server key-value pairs via `SetKeyValue`.
///
/// This mirrors `Sandbox.Game.MyCachedServerItem.SendSettingsToSteam`.
pub fn encode_rules(data: &MyCachedServerItem_MyServerData) -> HashMap<String, String> {
    use proto_rs::ProtoEncode;

    let proto_bytes = data.encode_to_vec();
    let compressed = compression::compress(&proto_bytes);

    let mut rules = HashMap::new();
    rules.insert(RULE_KEY.to_owned(), compressed.len().to_string());

    let num_chunks = compressed.len().div_ceil(RULE_CHUNK_SIZE);
    for i in 0..num_chunks {
        let start = i * RULE_CHUNK_SIZE;
        let end = (start + RULE_CHUNK_SIZE).min(compressed.len());
        let chunk_b64 = BASE64_STANDARD.encode(&compressed[start..end]);
        rules.insert(format!("{RULE_KEY}{i}"), chunk_b64);
    }

    rules
}

// ---------------------------------------------------------------------------
// Decode
// ---------------------------------------------------------------------------

/// Decode server data from a Steam rules map.
///
/// Expects rules produced by [`encode_rules`] (or the game's
/// `SendSettingsToSteam`). Returns the deserialized `MyServerData`.
///
/// This mirrors `Sandbox.Game.MyCachedServerItem.DeserializeSettings`.
pub fn decode_rules(
    rules: &HashMap<String, String>,
) -> Result<MyCachedServerItem_MyServerData, ServerDataError> {
    use proto_rs::ProtoDecode;

    let size_str = rules.get(RULE_KEY).ok_or(ServerDataError::MissingSizeKey)?;
    let total_len: usize = size_str
        .parse()
        .map_err(ServerDataError::InvalidSizeValue)?;

    let mut compressed = vec![0u8; total_len];
    let num_chunks = total_len.div_ceil(RULE_CHUNK_SIZE);

    for i in 0..num_chunks {
        let key = format!("{RULE_KEY}{i}");
        let chunk_b64 = rules
            .get(&key)
            .ok_or_else(|| ServerDataError::MissingChunkKey(key))?;
        let chunk = BASE64_STANDARD.decode(chunk_b64)?;
        let start = i * RULE_CHUNK_SIZE;
        compressed[start..start + chunk.len()].copy_from_slice(&chunk);
    }

    let decompressed = compression::decompress(&compressed)?;
    let server_data = MyCachedServerItem_MyServerData::decode(
        decompressed.as_slice(),
        proto_rs::DecodeContext::default(),
    )?;
    Ok(server_data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_default() {
        let original = MyCachedServerItem_MyServerData::default();
        let rules = encode_rules(&original);
        let decoded = decode_rules(&rules).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn round_trip_with_data() {
        let mut data = MyCachedServerItem_MyServerData::default();
        data.description = "Test Server".into();
        data.experimental_mode = true;
        data.used_services.push("Steam".into());

        let rules = encode_rules(&data);
        let decoded = decode_rules(&rules).unwrap();
        assert_eq!(data, decoded);
    }

    #[test]
    fn chunk_keys_are_sequential() {
        let mut data = MyCachedServerItem_MyServerData::default();
        // Make description large enough to require multiple chunks
        data.description = "A".repeat(500);

        let rules = encode_rules(&data);

        let total_len: usize = rules["sc"].parse().unwrap();
        let expected_chunks = total_len.div_ceil(RULE_CHUNK_SIZE);

        for i in 0..expected_chunks {
            assert!(
                rules.contains_key(&format!("sc{i}")),
                "missing chunk key sc{i}"
            );
        }
        // +1 for the "sc" size key itself
        assert_eq!(rules.len(), expected_chunks + 1);
    }

    #[test]
    fn decode_missing_sc_key() {
        let rules = HashMap::new();
        assert!(matches!(
            decode_rules(&rules),
            Err(ServerDataError::MissingSizeKey)
        ));
    }

    #[test]
    fn decode_invalid_sc_value() {
        let mut rules = HashMap::new();
        rules.insert("sc".into(), "not_a_number".into());
        assert!(matches!(
            decode_rules(&rules),
            Err(ServerDataError::InvalidSizeValue(_))
        ));
    }

    #[test]
    fn decode_missing_chunk_key() {
        let mut rules = HashMap::new();
        rules.insert("sc".into(), "100".into());
        // "sc0" is missing
        assert!(matches!(
            decode_rules(&rules),
            Err(ServerDataError::MissingChunkKey(_))
        ));
    }
}
