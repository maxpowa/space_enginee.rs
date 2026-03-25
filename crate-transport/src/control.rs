//! Control packets for Space Engineers server management.
//!
//! These packets handle administrative operations like kicking, banning,
//! and password authentication. They are sent on a separate P2P channel
//! from replication packets.
//!
//! # Protocol
//!
//! Control messages are serialized via `MyMultiplayerBase.SendControlMessage<T>`:
//! - Header: `u16` packet ID ([`MyControlMessageEnum`])
//! - Body: Message-specific payload (protobuf-serialized structs)
//!
//! # Game Source References
//!
//! - `Sandbox.Engine.Multiplayer.MyControlMessageEnum` - Packet ID enum
//! - `Sandbox.Engine.Multiplayer.MyMultiplayerBase.SendControlMessage<T>` - Send method
//! - `Sandbox.Engine.Multiplayer.MyMultiplayerBase.ControlMessageReceived` - Receive handler

use deku::prelude::*;
use space_engineers_compat::VarBytes;

/// Control packet for server management operations.
///
/// See: `Sandbox.Engine.Multiplayer.MyMultiplayerBase.ControlMessageReceived`
#[derive(Debug, PartialEq, DekuRead, DekuWrite)]
#[deku(ctx = "size: usize")]
pub struct ControlPacket {
    /// Control packet type identifier
    pub packet_id: ControlPacketId,
    /// Packet payload (depends on packet_id)
    #[deku(ctx = "*packet_id, size - deku::byte_offset")]
    pub data: ControlPayload,
}

/// Control packet payload variants.
///
/// See: `Sandbox.Engine.Multiplayer.MyMultiplayerBase.SendControlMessage<T>`
#[derive(Debug, PartialEq, DekuRead, DekuWrite)]
#[deku(ctx = "identifier: ControlPacketId, _size: usize", id = "identifier")]
pub enum ControlPayload {
    /// Kick a client from the server.
    ///
    /// See: `Sandbox.Engine.Multiplayer.MyMultiplayerServerBase.KickClient`
    #[deku(id = "ControlPacketId::Kick")]
    Kick {
        /// Steam ID of the client to kick
        client_id: u64,
    },
    /// Notify that a client has disconnected.
    ///
    /// See: `Sandbox.Engine.Multiplayer.MyDedicatedServerBase.DisconnectClient`
    /// See: `Sandbox.Engine.Multiplayer.MyMultiplayerClient.CloseClient`
    #[deku(id = "ControlPacketId::Disconnected")]
    Disconnected {
        /// Steam ID of the disconnected client
        client_id: u64,
    },
    /// Ban a client from the server.
    ///
    /// See: `Sandbox.Engine.Multiplayer.MyDedicatedServerBase.BanClient`
    #[deku(id = "ControlPacketId::Ban")]
    Ban {
        /// Steam ID of the client to ban
        client_id: u64,
    },
    /// Send password hash for server authentication.
    ///
    /// The hash is PBKDF2-derived: `Rfc2898DeriveBytes(password, salt, 10000).GetBytes(20)`
    ///
    /// See: `Sandbox.Engine.Multiplayer.MyMultiplayerClient.SendPasswordHash`
    /// See: `Sandbox.Engine.Multiplayer.MyControlSendPasswordHashMsg`
    #[deku(id = "ControlPacketId::SendPasswordHash")]
    SendPasswordHash {
        /// PBKDF2-SHA1 hash of the server password (20 bytes)
        password_hash: VarBytes,
    },
}

/// Control packet type identifier.
///
/// See: `Sandbox.Engine.Multiplayer.MyControlMessageEnum`
#[derive(Clone, Copy, Debug, PartialEq, DekuRead, DekuWrite)]
#[repr(u16)]
#[deku(id_type = "u16")]
pub enum ControlPacketId {
    /// Kick packet
    Kick = 0,
    /// Disconnected notification
    Disconnected = 1,
    /// Ban packet
    Ban = 2,
    /// Password hash packet
    SendPasswordHash = 3,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::PacketFrame;
    use std::io::BufReader;

    #[test]
    fn test_control_packet_disconnected() {
        // Real sample: Disconnected packet for Steam ID 76561198094742934
        let bytes: Vec<u8> = vec![
            206, 0, 255, 255, 255, 255, 0, 1, 1, 0, 150, 245, 3, 8, 1, 0, 16, 1,
        ];

        let size = bytes.len();
        let cursor = std::io::Cursor::new(bytes);
        let mut reader = Reader::new(BufReader::new(cursor));

        let result = PacketFrame::<ControlPacket>::from_reader_with_ctx(&mut reader, size);
        assert!(result.is_ok());
        let packet_frame = result.unwrap();
        let packet = packet_frame.inner;

        assert_eq!(packet.packet_id, ControlPacketId::Disconnected);
        if let ControlPayload::Disconnected { client_id } = packet.data {
            assert_eq!(client_id, 76561198094742934);
        } else {
            panic!("Expected Disconnected packet");
        }
    }
}
