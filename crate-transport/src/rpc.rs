//! RPC (Remote Procedure Call) packet handling for Space Engineers.
//!
//! This module provides RPC packet parsing with integrated schema support.
//! Event IDs are version-specific, but payloads are parsed into stable types.
//!
//! # Packet Format
//!
//! ```text
//! RpcPacket Layout:
//! ┌─────────────────────┬──────────────────────┐
//! │ network_id          │ Varint               │
//! │ blocked_by          │ Varint               │
//! │ event_id            │ u16                  │
//! │ position            │ Nullable<Vector3D>   │
//! │ payload             │ <varies by event>    │
//! │ terminator          │ u16 (0xC8B9)         │
//! └─────────────────────┴──────────────────────┘
//! ```
//!
//! Note: The payload has NO length prefix. Its format is determined by the
//! event type, and parsing continues until the terminator.
//!
//! # Usage
//!
//! ```ignore
//! use space_engineers_transport::{StaticRpcPacket, StaticEventPayload};
//!
//! // Parse with the embedded schema (fully typed)
//! let (_, packet) = StaticRpcPacket::from_bytes((data, 0))?;
//!
//! match &packet.payload {
//!     StaticEventPayload::OnChatMessageReceived_Server(msg) => { /* handle */ },
//!     StaticEventPayload::ModMessageServerReliable(msg) => { /* ... */ },
//!     _ => { /* ... */ },
//! }
//! ```

use std::io::{Read, Seek, Write};

use deku::bitvec::BitField;
use deku::ctx::Order;
use deku::prelude::*;
use deku::reader::Reader;
use deku::writer::Writer;
use space_engineers_compat::{BitAligned, Nullable};
use space_engineers_sys::math::Vector3D;

use crate::packet::TERMINATOR;
use crate::protocol::{SchemaError, StaticEventPayload, StaticEventType, Version};
use crate::replication::NetworkId;

// =============================================================================
// Raw RPC Packet (for multi-version or manual parsing)
// =============================================================================

/// Raw RPC packet that preserves payload bytes without parsing.
///
/// Use this when:
/// - Working with multiple game versions
/// - You need to inspect the raw payload
/// - You want to defer payload parsing
///
/// For fully-typed parsing with the embedded schema, use [`StaticRpcPacket`].
#[derive(Debug, Clone, PartialEq)]
pub struct RawRpcPacket {
    /// Network ID of the target object (0 = static/global event)
    pub network_id: NetworkId,
    /// Network ID that blocks this event (for ordering)
    pub blocked_by_network_id: NetworkId,
    /// Raw event identifier (version-specific, use schema to resolve)
    pub event_id: u16,
    /// Optional position for proximity-based events (uses default as sentinel for "no position")
    pub position: Nullable<Vector3D>,
    /// Raw payload bytes
    pub payload: Vec<u8>,
}

impl DekuReader<'_, ()> for RawRpcPacket {
    fn from_reader_with_ctx<R: Read + Seek>(
        reader: &mut Reader<R>,
        _ctx: (),
    ) -> Result<Self, DekuError> {
        let network_id = NetworkId::from_reader_with_ctx(reader, ())?;
        let blocked_by_network_id = NetworkId::from_reader_with_ctx(reader, ())?;
        let event_id: BitAligned<u16> = BitAligned::from_reader_with_ctx(reader, ())?;
        let position: Nullable<Vector3D> = Nullable::from_reader_with_ctx(reader, ())?;

        // Read all remaining bits into a buffer.
        // After Nullable<Vector3D> (1-bit presence flag), we may not be byte-aligned.
        // The terminator is written at the current bit position, not necessarily byte-aligned.
        // After the terminator, there are 0-7 padding bits to reach a byte boundary.
        // The padding bits are NOT guaranteed to be zeros - they're whatever junk is in the stream.
        let mut all_bits: Vec<bool> = Vec::new();
        loop {
            match reader.read_bits(1, Order::Lsb0) {
                Ok(Some(bits)) => all_bits.push(bits.load_le::<u8>() != 0),
                Ok(None) => break,
                Err(DekuError::Incomplete(_)) => break,
                Err(e) => return Err(e),
            }
        }

        // Need at least 16 bits for the terminator
        if all_bits.len() < 16 {
            return Err(DekuError::Assertion(
                "RPC packet too short - missing terminator".into(),
            ));
        }

        // Search for the terminator (0xC8B9) from the end.
        // The terminator is followed by 0-7 padding bits.
        // We search backwards from the last possible position (allowing up to 7 padding bits).
        let mut term_pos = None;
        let max_padding = 7.min(all_bits.len().saturating_sub(16));

        for padding in 0..=max_padding {
            let candidate_end = all_bits.len() - padding;
            if candidate_end < 16 {
                break;
            }
            let candidate_start = candidate_end - 16;

            // Extract the candidate terminator value
            let mut terminator: u16 = 0;
            for (i, &bit) in all_bits[candidate_start..candidate_end].iter().enumerate() {
                if bit {
                    terminator |= 1 << i;
                }
            }

            if terminator == TERMINATOR {
                term_pos = Some(candidate_start);
                break;
            }
        }

        let term_start = term_pos.ok_or_else(|| {
            // For error reporting, show what we found at each position
            let mut found_values: Vec<String> = Vec::new();
            for padding in 0..=max_padding {
                let candidate_end = all_bits.len() - padding;
                if candidate_end < 16 {
                    break;
                }
                let candidate_start = candidate_end - 16;
                let mut found: u16 = 0;
                for (i, &bit) in all_bits[candidate_start..candidate_end].iter().enumerate() {
                    if bit {
                        found |= 1 << i;
                    }
                }
                found_values.push(format!("0x{:04X}", found));
            }
            DekuError::Assertion(
                format!(
                    "Invalid RPC terminator: expected 0x{:04X}, searched {} positions: [{}]",
                    TERMINATOR,
                    max_padding + 1,
                    found_values.join(", ")
                )
                .into(),
            )
        })?;

        // Convert payload bits to bytes (the bits before the terminator)
        let payload_bits = &all_bits[..term_start];
        let payload = bits_to_bytes(payload_bits);

        Ok(RawRpcPacket {
            network_id,
            blocked_by_network_id,
            event_id: event_id.0,
            position,
            payload,
        })
    }
}

/// Convert a slice of bits (LSB first within each byte) to bytes
fn bits_to_bytes(bits: &[bool]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity((bits.len() + 7) / 8);
    for chunk in bits.chunks(8) {
        let mut byte: u8 = 0;
        for (i, &bit) in chunk.iter().enumerate() {
            if bit {
                byte |= 1 << i;
            }
        }
        bytes.push(byte);
    }
    bytes
}

impl DekuWriter<()> for RawRpcPacket {
    fn to_writer<W: Write + Seek>(
        &self,
        writer: &mut Writer<W>,
        _ctx: (),
    ) -> Result<(), DekuError> {
        self.network_id.to_writer(writer, ())?;
        self.blocked_by_network_id.to_writer(writer, ())?;
        BitAligned(self.event_id).to_writer(writer, ())?;
        self.position.to_writer(writer, ())?;

        // Write payload bytes
        for &byte in &self.payload {
            BitAligned(byte).to_writer(writer, ())?;
        }

        // Write terminator
        BitAligned(TERMINATOR).to_writer(writer, ())?;

        Ok(())
    }
}

impl RawRpcPacket {
    /// Parse from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, DekuError> {
        let mut cursor = std::io::Cursor::new(data);
        let mut reader = Reader::new(&mut cursor);
        Self::from_reader_with_ctx(&mut reader, ())
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, DekuError> {
        let mut out_buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut out_buf);
        let mut writer = Writer::new(&mut cursor);
        self.to_writer(&mut writer, ())?;
        writer.finalize()?;
        Ok(out_buf)
    }

    /// Returns true if this is a static (global) event (network_id = 0).
    #[inline]
    pub fn is_static_event(&self) -> bool {
        self.network_id.0 == 0
    }

    /// Returns true if this is an instance (object-specific) event (network_id > 0).
    #[inline]
    pub fn is_instance_event(&self) -> bool {
        self.network_id.0 != 0
    }

    /// Resolve the static event type identity using the schema.
    pub fn resolve_static_event(
        &self,
        schema: &Version,
    ) -> Result<Option<StaticEventType>, RpcError> {
        if !self.is_static_event() {
            return Err(RpcError::NotStaticEvent);
        }

        let event_hash = match schema.try_decode_static_event_id(self.event_id) {
            Some(hash) => hash,
            None => return Ok(None),
        };

        Ok(StaticEventType::from_hash(event_hash))
    }

    /// Parse the static event payload using the schema.
    pub fn parse_static_event(&self, schema: &Version) -> Result<StaticEventPayload, RpcError> {
        if !self.is_static_event() {
            return Err(RpcError::NotStaticEvent);
        }

        let event_hash = schema
            .try_decode_static_event_id(self.event_id)
            .ok_or(RpcError::UnknownEventId(self.event_id))?;

        crate::protocol::parse_static_event(event_hash, &self.payload)
            .map_err(RpcError::PayloadParse)
    }

    /// Parse the static event payload using the embedded schema.
    pub fn parse_static_event_embedded(&self) -> Result<StaticEventPayload, RpcError> {
        self.parse_static_event(&Version::embedded())
    }

    /// Get the stable event hash for an instance event.
    pub fn resolve_instance_event_hash(
        &self,
        schema: &Version,
        type_hash: i32,
    ) -> Result<i32, RpcError> {
        schema
            .try_decode_instance_event_id(type_hash, self.event_id)
            .ok_or(RpcError::UnknownInstanceEvent {
                type_hash,
                event_id: self.event_id,
            })
    }
}

// =============================================================================
// Typed Static RPC Packet (for embedded schema version)
// =============================================================================

/// Fully-typed static RPC packet using the embedded schema.
///
/// This parses the payload directly into [`StaticEventPayload`] based on the
/// event ID mappings in the embedded schema.
///
/// For multi-version support, use [`RawRpcPacket`] instead.
#[derive(Debug, Clone, PartialEq)]
pub struct StaticRpcPacket {
    /// Network ID (should be 0 for static events)
    pub network_id: NetworkId,
    /// Network ID that blocks this event
    pub blocked_by_network_id: NetworkId,
    /// Event ID (from embedded schema version)
    pub event_id: u16,
    /// Position for proximity events (default = no position)
    pub position: Nullable<Vector3D>,
    /// Parsed payload
    pub payload: StaticEventPayload,
}

impl DekuReader<'_, ()> for StaticRpcPacket {
    fn from_reader_with_ctx<R: Read + Seek>(
        reader: &mut Reader<R>,
        _ctx: (),
    ) -> Result<Self, DekuError> {
        // Parse as raw first
        let raw = RawRpcPacket::from_reader_with_ctx(reader, ())?;

        // Resolve and parse payload
        let schema = Version::embedded();
        let event_hash = schema
            .try_decode_static_event_id(raw.event_id)
            .ok_or_else(|| {
                DekuError::Assertion(
                    format!(
                        "Unknown static event ID {} in embedded schema",
                        raw.event_id
                    )
                    .into(),
                )
            })?;

        let payload = crate::protocol::parse_static_event(event_hash, &raw.payload)?;

        Ok(StaticRpcPacket {
            network_id: raw.network_id,
            blocked_by_network_id: raw.blocked_by_network_id,
            event_id: raw.event_id,
            position: raw.position,
            payload,
        })
    }
}

impl StaticRpcPacket {
    /// Parse from bytes using the embedded schema.
    pub fn from_bytes(data: &[u8]) -> Result<Self, DekuError> {
        let mut cursor = std::io::Cursor::new(data);
        let mut reader = Reader::new(&mut cursor);
        Self::from_reader_with_ctx(&mut reader, ())
    }

    /// Returns true if this is a static (global) event (network_id = 0).
    #[inline]
    pub fn is_static_event(&self) -> bool {
        self.network_id.0 == 0
    }

    /// Get the stable event type identity.
    pub fn event_type(&self) -> Option<StaticEventType> {
        let schema = Version::embedded();
        schema
            .try_decode_static_event_id(self.event_id)
            .and_then(StaticEventType::from_hash)
    }
}

// =============================================================================
// Error Type
// =============================================================================

/// Error type for RPC operations.
#[derive(Debug)]
pub enum RpcError {
    /// Attempted to parse a static event, but network_id != 0.
    NotStaticEvent,
    /// The event ID was not found in the schema.
    UnknownEventId(u16),
    /// The instance event ID was not found for the given type.
    UnknownInstanceEvent { type_hash: i32, event_id: u16 },
    /// Failed to parse the event payload.
    PayloadParse(deku::DekuError),
    /// Schema lookup error.
    Schema(SchemaError),
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RpcError::NotStaticEvent => write!(f, "not a static event (network_id != 0)"),
            RpcError::UnknownEventId(id) => write!(f, "unknown event ID: {}", id),
            RpcError::UnknownInstanceEvent {
                type_hash,
                event_id,
            } => {
                write!(
                    f,
                    "unknown instance event {} for type hash {}",
                    event_id, type_hash
                )
            }
            RpcError::PayloadParse(e) => write!(f, "payload parse error: {}", e),
            RpcError::Schema(e) => write!(f, "schema error: {}", e),
        }
    }
}

impl std::error::Error for RpcError {}

impl From<SchemaError> for RpcError {
    fn from(e: SchemaError) -> Self {
        RpcError::Schema(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use space_engineers_compat::Varint;

    #[test]
    fn test_rpc_round_trip_no_position() {
        // Create a packet with no position (1 bit for false)
        let original = RawRpcPacket {
            network_id: Varint(0), // static event
            blocked_by_network_id: Varint(0),
            event_id: 42,
            position: Nullable::none(), // 1 bit = false, stream not byte-aligned after this
            payload: vec![0xAB, 0xCD, 0xEF], // 3 bytes of payload
        };

        // Serialize
        let bytes = original.to_bytes().expect("serialization should succeed");

        // Parse back
        let parsed = RawRpcPacket::from_bytes(&bytes).expect("parsing should succeed");

        assert_eq!(parsed.network_id.0, original.network_id.0);
        assert_eq!(
            parsed.blocked_by_network_id.0,
            original.blocked_by_network_id.0
        );
        assert_eq!(parsed.event_id, original.event_id);
        assert!(parsed.position.is_none());
        assert_eq!(parsed.payload, original.payload);
    }

    #[test]
    fn test_rpc_round_trip_with_position() {
        use space_engineers_compat::BitAligned;
        use space_engineers_sys::math::Vector3D;

        let pos = Vector3D {
            x: BitAligned(1.0),
            y: BitAligned(2.0),
            z: BitAligned(3.0),
        };
        let original = RawRpcPacket {
            network_id: Varint(123),
            blocked_by_network_id: Varint(456),
            event_id: 999,
            position: Nullable::some(pos),
            payload: vec![0x11, 0x22],
        };

        let bytes = original.to_bytes().expect("serialization should succeed");
        let parsed = RawRpcPacket::from_bytes(&bytes).expect("parsing should succeed");

        assert_eq!(parsed.network_id.0, original.network_id.0);
        assert_eq!(
            parsed.blocked_by_network_id.0,
            original.blocked_by_network_id.0
        );
        assert_eq!(parsed.event_id, original.event_id);
        assert!(parsed.position.is_some());
        let parsed_pos = parsed.position.as_ref().unwrap();
        assert_eq!(parsed_pos.x.0, 1.0);
        assert_eq!(parsed_pos.y.0, 2.0);
        assert_eq!(parsed_pos.z.0, 3.0);
        assert_eq!(parsed.payload, original.payload);
    }

    #[test]
    fn test_rpc_empty_payload() {
        let original = RawRpcPacket {
            network_id: Varint(0),
            blocked_by_network_id: Varint(0),
            event_id: 0,
            position: Nullable::none(),
            payload: vec![],
        };

        let bytes = original.to_bytes().expect("serialization should succeed");
        let parsed = RawRpcPacket::from_bytes(&bytes).expect("parsing should succeed");

        assert_eq!(parsed.payload.len(), 0);
    }
}
