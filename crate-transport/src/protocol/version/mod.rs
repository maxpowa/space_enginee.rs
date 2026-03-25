//! Runtime schema loading for version-specific type table mappings.
//!
//! Space Engineers assigns numeric indices to types at runtime, and these indices
//! change between game versions. This module provides runtime lookup tables to
//! convert between version-specific indices and stable type hashes.
//!
//! # Usage
//!
//! ```ignore
//! use space_engineers_transport::Version;
//!
//! // Use the embedded schema (recommended for most cases)
//! let schema = Version::embedded();
//! println!("Built against game version: {}", schema.game_version);
//!
//! // Or load a specific version's schema at runtime
//! let schema = Version::load("schemas/schema_v1205026.json")?;
//!
//! // Decode a type index from a packet
//! let type_hash = schema.decode_type_index(61)?;
//!
//! // Look up the stable identity
//! let type_id = ReplicatedType::from_hash(type_hash);
//! ```

use std::collections::HashMap;
use std::io;
use std::path::Path;

use serde::Deserialize;

/// Embedded schema JSON, generated at build time by codegen.
/// This is the schema for the game version the crate was built against.
const EMBEDDED_SCHEMA: &str = include_str!("embedded_schema.json");

/// Error type for schema operations.
#[derive(Debug)]
pub enum SchemaError {
    Io(io::Error),
    Json(serde_json::Error),
    UnknownTypeIndex(u16),
    UnknownTypeHash(i32),
    UnknownEventId { type_hash: i32, event_id: u16 },
}

impl From<io::Error> for SchemaError {
    fn from(e: io::Error) -> Self {
        SchemaError::Io(e)
    }
}

impl From<serde_json::Error> for SchemaError {
    fn from(e: serde_json::Error) -> Self {
        SchemaError::Json(e)
    }
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaError::Io(e) => write!(f, "IO error: {}", e),
            SchemaError::Json(e) => write!(f, "JSON error: {}", e),
            SchemaError::UnknownTypeIndex(idx) => write!(f, "Unknown type index: {}", idx),
            SchemaError::UnknownTypeHash(hash) => write!(f, "Unknown type hash: {}", hash),
            SchemaError::UnknownEventId { type_hash, event_id } => {
                write!(f, "Unknown event id {} for type hash {}", event_id, type_hash)
            }
        }
    }
}

impl std::error::Error for SchemaError {}

/// Raw schema data as stored in JSON.
#[derive(Debug, Deserialize)]
struct RawSchema {
    game_version: String,
    #[allow(dead_code)]
    generated_at: String,
    types: Vec<RawTypeEntry>,
    static_events: Vec<RawStaticEventEntry>,
    instance_events: Vec<RawInstanceEventGroup>,
}

#[derive(Debug, Deserialize)]
struct RawTypeEntry {
    index: u16,
    hash: i32,
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    is_replicated: bool,
}

#[derive(Debug, Deserialize)]
struct RawStaticEventEntry {
    id: u16,
    hash: i32,
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    declaring_type_hash: i32,
    #[allow(dead_code)]
    is_reliable: bool,
}

#[derive(Debug, Deserialize)]
struct RawInstanceEventGroup {
    type_hash: i32,
    #[allow(dead_code)]
    type_name: String,
    events: Vec<RawInstanceEventEntry>,
}

#[derive(Debug, Deserialize)]
struct RawInstanceEventEntry {
    id: u16,
    hash: i32,
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    is_reliable: bool,
}

/// Instance event lookup table for a single type.
#[derive(Debug, Default)]
pub struct InstanceEventSchema {
    /// Maps event ID (version-specific) to event hash (stable)
    event_id_to_hash: Vec<i32>,
    /// Maps event hash (stable) to event ID (version-specific)
    event_hash_to_id: HashMap<i32, u16>,
}

impl InstanceEventSchema {
    /// Decode a version-specific event ID to stable hash.
    pub fn decode_event_id(&self, event_id: u16) -> Option<i32> {
        self.event_id_to_hash.get(event_id as usize).copied()
    }

    /// Encode a stable event hash to version-specific ID.
    pub fn encode_event_hash(&self, hash: i32) -> Option<u16> {
        self.event_hash_to_id.get(&hash).copied()
    }
}

/// Runtime schema for a specific game version.
///
/// Provides bidirectional lookup between version-specific indices and stable hashes.
#[derive(Debug)]
pub struct Version {
    /// Game version string
    pub game_version: String,

    /// Maps type index (version-specific) to type hash (stable)
    type_index_to_hash: Vec<i32>,
    /// Maps type hash (stable) to type index (version-specific)
    type_hash_to_index: HashMap<i32, u16>,

    /// Maps static event ID (version-specific) to event hash (stable)
    static_event_id_to_hash: Vec<i32>,
    /// Maps static event hash (stable) to event ID (version-specific)
    static_event_hash_to_id: HashMap<i32, u16>,

    /// Instance event schemas, keyed by type hash
    instance_events: HashMap<i32, InstanceEventSchema>,
}

impl Version {
    /// Load a schema from a JSON file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, SchemaError> {
        let file = std::fs::File::open(path)?;
        let raw: RawSchema = serde_json::from_reader(file)?;

        let mut schema = Self {
            game_version: raw.game_version,
            type_index_to_hash: Vec::new(),
            type_hash_to_index: HashMap::new(),
            static_event_id_to_hash: Vec::new(),
            static_event_hash_to_id: HashMap::new(),
            instance_events: HashMap::new(),
        };

        // Build type lookup tables
        for t in raw.types {
            while schema.type_index_to_hash.len() <= t.index as usize {
                schema.type_index_to_hash.push(0);
            }
            schema.type_index_to_hash[t.index as usize] = t.hash;
            schema.type_hash_to_index.insert(t.hash, t.index);
        }

        // Build static event lookup tables
        for e in raw.static_events {
            while schema.static_event_id_to_hash.len() <= e.id as usize {
                schema.static_event_id_to_hash.push(0);
            }
            schema.static_event_id_to_hash[e.id as usize] = e.hash;
            schema.static_event_hash_to_id.insert(e.hash, e.id);
        }

        // Build instance event lookup tables per type
        for group in raw.instance_events {
            let mut instance_schema = InstanceEventSchema::default();

            for e in group.events {
                while instance_schema.event_id_to_hash.len() <= e.id as usize {
                    instance_schema.event_id_to_hash.push(0);
                }
                instance_schema.event_id_to_hash[e.id as usize] = e.hash;
                instance_schema.event_hash_to_id.insert(e.hash, e.id);
            }

            schema.instance_events.insert(group.type_hash, instance_schema);
        }

        Ok(schema)
    }

    /// Load a schema from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, SchemaError> {
        let raw: RawSchema = serde_json::from_str(json)?;

        // Reuse the same building logic
        let mut schema = Self {
            game_version: raw.game_version,
            type_index_to_hash: Vec::new(),
            type_hash_to_index: HashMap::new(),
            static_event_id_to_hash: Vec::new(),
            static_event_hash_to_id: HashMap::new(),
            instance_events: HashMap::new(),
        };

        for t in raw.types {
            while schema.type_index_to_hash.len() <= t.index as usize {
                schema.type_index_to_hash.push(0);
            }
            schema.type_index_to_hash[t.index as usize] = t.hash;
            schema.type_hash_to_index.insert(t.hash, t.index);
        }

        for e in raw.static_events {
            while schema.static_event_id_to_hash.len() <= e.id as usize {
                schema.static_event_id_to_hash.push(0);
            }
            schema.static_event_id_to_hash[e.id as usize] = e.hash;
            schema.static_event_hash_to_id.insert(e.hash, e.id);
        }

        for group in raw.instance_events {
            let mut instance_schema = InstanceEventSchema::default();

            for e in group.events {
                while instance_schema.event_id_to_hash.len() <= e.id as usize {
                    instance_schema.event_id_to_hash.push(0);
                }
                instance_schema.event_id_to_hash[e.id as usize] = e.hash;
                instance_schema.event_hash_to_id.insert(e.hash, e.id);
            }

            schema.instance_events.insert(group.type_hash, instance_schema);
        }

        Ok(schema)
    }

    /// Load the embedded schema that was built into the crate.
    ///
    /// This returns the schema for the game version the crate was compiled against.
    /// Use this when you don't need to load a specific version's schema at runtime.
    ///
    /// # Panics
    ///
    /// Panics if the embedded schema is malformed (should never happen with
    /// codegen-generated schemas).
    pub fn embedded() -> Self {
        Self::from_json(EMBEDDED_SCHEMA).expect("embedded schema should be valid")
    }

    /// Get the game version of the embedded schema without fully parsing it.
    pub fn embedded_version() -> &'static str {
        // Parse just enough to extract the version
        // The schema starts with {"game_version":"XXXXXXX",...
        let start = EMBEDDED_SCHEMA.find(r#""game_version":""#).unwrap() + 16;
        let end = start + EMBEDDED_SCHEMA[start..].find('"').unwrap();
        &EMBEDDED_SCHEMA[start..end]
    }

    // -------------------------------------------------------------------------
    // Type table operations
    // -------------------------------------------------------------------------

    /// Decode a version-specific type index to stable type hash.
    pub fn decode_type_index(&self, index: u16) -> Result<i32, SchemaError> {
        self.type_index_to_hash
            .get(index as usize)
            .copied()
            .filter(|&h| h != 0)
            .ok_or(SchemaError::UnknownTypeIndex(index))
    }

    /// Try to decode a type index, returning None for unknown indices.
    pub fn try_decode_type_index(&self, index: u16) -> Option<i32> {
        self.type_index_to_hash
            .get(index as usize)
            .copied()
            .filter(|&h| h != 0)
    }

    /// Encode a stable type hash to version-specific index.
    pub fn encode_type_hash(&self, hash: i32) -> Result<u16, SchemaError> {
        self.type_hash_to_index
            .get(&hash)
            .copied()
            .ok_or(SchemaError::UnknownTypeHash(hash))
    }

    /// Try to encode a type hash, returning None for unknown hashes.
    pub fn try_encode_type_hash(&self, hash: i32) -> Option<u16> {
        self.type_hash_to_index.get(&hash).copied()
    }

    /// Get the number of types in the schema.
    pub fn type_count(&self) -> usize {
        self.type_index_to_hash.len()
    }

    // -------------------------------------------------------------------------
    // Static event operations
    // -------------------------------------------------------------------------

    /// Decode a version-specific static event ID to stable event hash.
    pub fn decode_static_event_id(&self, event_id: u16) -> Result<i32, SchemaError> {
        self.static_event_id_to_hash
            .get(event_id as usize)
            .copied()
            .filter(|&h| h != 0)
            .ok_or(SchemaError::UnknownEventId {
                type_hash: 0,
                event_id,
            })
    }

    /// Try to decode a static event ID, returning None for unknown IDs.
    pub fn try_decode_static_event_id(&self, event_id: u16) -> Option<i32> {
        self.static_event_id_to_hash
            .get(event_id as usize)
            .copied()
            .filter(|&h| h != 0)
    }

    /// Encode a stable static event hash to version-specific ID.
    pub fn encode_static_event_hash(&self, hash: i32) -> Option<u16> {
        self.static_event_hash_to_id.get(&hash).copied()
    }

    /// Get the number of static events in the schema.
    pub fn static_event_count(&self) -> usize {
        self.static_event_id_to_hash.len()
    }

    // -------------------------------------------------------------------------
    // Instance event operations
    // -------------------------------------------------------------------------

    /// Get the instance event schema for a given type hash.
    pub fn instance_events(&self, type_hash: i32) -> Option<&InstanceEventSchema> {
        self.instance_events.get(&type_hash)
    }

    /// Decode an instance event ID for a given type.
    pub fn decode_instance_event_id(
        &self,
        type_hash: i32,
        event_id: u16,
    ) -> Result<i32, SchemaError> {
        self.instance_events
            .get(&type_hash)
            .and_then(|schema| schema.decode_event_id(event_id))
            .ok_or(SchemaError::UnknownEventId { type_hash, event_id })
    }

    /// Try to decode an instance event ID, returning None for unknown.
    pub fn try_decode_instance_event_id(&self, type_hash: i32, event_id: u16) -> Option<i32> {
        self.instance_events
            .get(&type_hash)
            .and_then(|schema| schema.decode_event_id(event_id))
    }

    /// Encode an instance event hash to version-specific ID for a given type.
    pub fn encode_instance_event_hash(&self, type_hash: i32, event_hash: i32) -> Option<u16> {
        self.instance_events
            .get(&type_hash)
            .and_then(|schema| schema.encode_event_hash(event_hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SCHEMA: &str = r#"{
        "game_version": "1205026",
        "generated_at": "2024-01-01T00:00:00Z",
        "types": [
            {"index": 0, "hash": 12345, "name": "TestType1", "is_replicated": true},
            {"index": 1, "hash": 67890, "name": "TestType2", "is_replicated": false}
        ],
        "static_events": [
            {"id": 0, "hash": 11111, "name": "TestEvent1", "declaring_type_hash": 12345, "is_reliable": true}
        ],
        "instance_events": [
            {
                "type_hash": 12345,
                "type_name": "TestType1",
                "events": [
                    {"id": 0, "hash": 22222, "name": "InstanceEvent1", "is_reliable": false}
                ]
            }
        ]
    }"#;

    #[test]
    fn test_load_schema() {
        let schema = Version::from_json(TEST_SCHEMA).unwrap();
        assert_eq!(schema.game_version, "1205026");
    }

    #[test]
    fn test_type_lookup() {
        let schema = Version::from_json(TEST_SCHEMA).unwrap();

        // Index -> Hash
        assert_eq!(schema.decode_type_index(0).unwrap(), 12345);
        assert_eq!(schema.decode_type_index(1).unwrap(), 67890);
        assert!(schema.decode_type_index(999).is_err());

        // Hash -> Index
        assert_eq!(schema.encode_type_hash(12345).unwrap(), 0);
        assert_eq!(schema.encode_type_hash(67890).unwrap(), 1);
        assert!(schema.encode_type_hash(99999).is_err());
    }

    #[test]
    fn test_static_event_lookup() {
        let schema = Version::from_json(TEST_SCHEMA).unwrap();

        assert_eq!(schema.decode_static_event_id(0).unwrap(), 11111);
        assert!(schema.decode_static_event_id(999).is_err());
    }

    #[test]
    fn test_instance_event_lookup() {
        let schema = Version::from_json(TEST_SCHEMA).unwrap();

        assert_eq!(
            schema.decode_instance_event_id(12345, 0).unwrap(),
            22222
        );
        assert!(schema.decode_instance_event_id(12345, 999).is_err());
        assert!(schema.decode_instance_event_id(99999, 0).is_err());
    }

    #[test]
    fn test_embedded_schema() {
        let schema = Version::embedded();
        // Should have a valid game version
        assert!(!schema.game_version.is_empty());
        // Should have types
        assert!(schema.type_count() > 0);
        // Should have static events
        assert!(schema.static_event_count() > 0);
    }

    #[test]
    fn test_embedded_version() {
        let version = Version::embedded_version();
        assert!(!version.is_empty());
        // Version should be numeric
        assert!(version.chars().all(|c| c.is_ascii_digit()));
    }
}
