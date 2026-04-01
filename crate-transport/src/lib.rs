//! Space Engineers network transport layer.
//!
//! This crate provides packet framing, replication, and RPC handling for
//! Space Engineers' network protocol.
//!
//! # Modules
//!
//! - [`packet`] - Low-level packet framing with checksums and fragmentation
//! - [`control`] - Server control packets (kick, ban, disconnect, password)
//! - [`replication`] - Replication packet types for client-server sync
//! - [`rpc`] - RPC (Remote Procedure Call) event handling
//! - [`protocol`] - Stable type/event identities and version schema (hash-based)
//!
//! # Multi-Version Support
//!
//! Event and type IDs change between game versions. Use one of:
//!
//! **Option 1: Fully-typed parsing** (single version, embedded schema)
//! ```ignore
//! use space_engineers_transport::{StaticRpcPacket, StaticEventPayload};
//!
//! let (_, packet) = StaticRpcPacket::from_bytes((data, 0))?;
//!
//! match &packet.payload {
//!     StaticEventPayload::OnChatMessageReceived_Server(msg) => { /* ... */ },
//!     StaticEventPayload::ModMessageServerReliable(msg) => { /* ... */ },
//!     _ => { /* ... */ },
//! }
//! ```
//!
//! **Option 2: Raw parsing with manual resolution** (multi-version)
//! ```ignore
//! use space_engineers_transport::{RawRpcPacket, Version};
//!
//! let schema = Version::load("schema_v1205026.json")?;
//! let (_, packet) = RawRpcPacket::from_bytes((data, 0))?;
//!
//! let payload = packet.parse_static_event(&schema)?;
//! ```

pub mod control;
pub mod packet;
pub mod replication;
pub mod rpc;
pub mod protocol;
pub mod server_data;

// Re-export commonly used types
pub use control::{ControlPacket, ControlPacketId, ControlPayload};
pub use packet::{MessageType, PacketFrame, MAGIC_NUMBER, TERMINATOR};
pub use replication::{
    ClientAckPacket, ClientConnectedPacket, ClientReadyPacket, ClientUpdatePacket, GameMode,
    JoinResult, JoinResultPacket, NetworkId, Packet, PacketId, ReplicationPacket, ServerDataPacket,
    WorldDataPacket,
};
pub use rpc::{RawRpcPacket, RpcError, StaticRpcPacket};
pub use protocol::{parse_static_event, ReplicatedType, StaticEventPayload, StaticEventType};
pub use protocol::{SchemaError, Version};
pub use server_data::{ServerDataError, encode_rules, decode_rules};
