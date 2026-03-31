//! Packet framing for Space Engineers network protocol.
//!
//! This module defines the outer packet frame structure that wraps all
//! replication packets. The frame includes checksums and support for
//! large packet fragmentation.

use crc::{Crc, CRC_32_ISO_HDLC};
use deku::no_std_io;
use deku::prelude::*;
use std::io::{Cursor, Seek, Write};

/// Magic number that identifies SE packets (cute little Keen logo-like character 'Î')
pub const MAGIC_NUMBER: u8 = 206;

/// Tamper-resistant packet terminator value
pub const TERMINATOR: u16 = 51385;

/// CRC-32 algorithm used for packet checksums
const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

/// Message type indicator for packet protection level.
#[derive(Clone, Copy, Debug, PartialEq, DekuRead, DekuWrite)]
#[repr(u8)]
#[deku(id_type = "u8")]
pub enum MessageType {
    /// Unprotected packet (checksum not verified)
    Unprotected = 0,
    /// Tamper-resistant packet (checksum verified)
    TamperResistant = 1,
}

/// Outer packet frame that wraps all replication data.
///
/// # Structure
/// ```text
/// +--------+-------------+-----------+---------------+------------------+------+
/// | Magic  | MessageType | Checksum  | PacketIndex   | PacketCount      | Data |
/// | 1 byte | 1 byte      | 4 bytes   | 1 byte        | 1 byte           | ...  |
/// +--------+-------------+-----------+---------------+------------------+------+
/// ```
///
/// See also:
/// - `Sandbox.Engine.Networking.MyNetworkWriter.SendAll`
/// - `Sandbox.Engine.Networking.MyReceiveQueue.ReceiveOne`
#[derive(Debug, PartialEq, DekuRead, DekuWrite)]
#[deku(ctx = "size: usize")]
pub struct PacketFrame<T>
where
    T: for<'a> DekuReader<'a, usize> + DekuWriter<usize>,
{
    /// Magic number -- looks similar to the Keen logo when viewed as a CP-437 character
    #[deku(assert_eq = "MAGIC_NUMBER")]
    pub magic_number: u8,

    /// Message type (protection level)
    pub message_type: MessageType,

    /// CRC-32 checksum of the packet data (packet_index, packet_count, inner)
    #[deku(writer = "self.write_checksum(deku::writer)")]
    checksum: u32,

    /// Sequential index of this packet (for large packets split across multiple frames)
    pub packet_index: u8,

    /// Total count of packets in this sequence (0 = single packet)
    pub packet_count: u8,

    /// Inner packet data (max ~1MB, if larger it should be split)
    #[deku(ctx = "size - deku::byte_offset")]
    pub inner: T,
}

impl<T: for<'a> DekuReader<'a, usize> + DekuWriter<usize>> PacketFrame<T> {
    /// Create a new packet frame with the given message type and inner data.
    pub fn new(message_type: MessageType, inner: T) -> Self {
        PacketFrame {
            magic_number: MAGIC_NUMBER,
            message_type,
            checksum: 0,
            packet_index: 1,
            packet_count: 0,
            inner,
        }
    }

    /// Create a new unprotected packet frame.
    pub fn unprotected(inner: T) -> Self {
        Self::new(MessageType::Unprotected, inner)
    }

    /// Create a new tamper-resistant packet frame.
    pub fn tamper_resistant(inner: T) -> Self {
        Self::new(MessageType::TamperResistant, inner)
    }

    /// Write the CRC-32 checksum for the packet.
    /// Note: CRC is written little-endian to match game's pointer-cast read.
    fn write_checksum<W: Write + Seek>(&self, writer: &mut Writer<W>) -> Result<(), DekuError> {
        let checksum = self.compute_checksum();
        // Write as little-endian to match game's *(int*) pointer read
        checksum.to_le_bytes().to_vec().to_writer(writer, ())
    }

    /// Compute the CRC-32 checksum for this packet.
    ///
    /// The checksum covers: `[packet_index, packet_count, inner_data...]`
    ///
    /// See: `Sandbox.Engine.Networking.MyReceiveQueue.CheckCrc`
    pub fn compute_checksum(&self) -> u32 {
        let mut buffer = Cursor::new(Vec::new());
        self.inner
            .to_writer(&mut Writer::new(&mut buffer), 1_000_000)
            .expect("Cannot serialize packet for checksum!");
        CRC32.checksum(
            &[
                &[self.packet_index],
                &[self.packet_count],
                buffer.into_inner().as_slice(),
            ]
            .concat(),
        )
    }

    /// Validate the packet checksum.
    ///
    /// Returns `true` if:
    /// - The packet is unprotected (checksum not required), or
    /// - The packet is tamper-resistant and the checksum matches
    ///
    /// Returns `false` if the packet is tamper-resistant but the checksum doesn't match.
    pub fn validate_checksum(&self) -> bool {
        match self.message_type {
            MessageType::Unprotected => true,
            MessageType::TamperResistant => self.checksum == self.compute_checksum(),
        }
    }

    /// Serialize the packet frame to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, DekuError> {
        let mut out_buf = Vec::new();
        let mut cursor = no_std_io::Cursor::new(&mut out_buf);
        let mut writer = Writer::new(&mut cursor);
        DekuWriter::to_writer(self, &mut writer, 1_000_000)?;
        writer.finalize()?;
        Ok(out_buf)
    }
}

#[cfg(test)]
mod tests {
    use crate::replication::{Packet, PacketId};
    use crate::{ReplicationPacket, StaticEventPayload};

    use super::*;

    #[test]
    fn test_message_type_round_trip() {
        let types = [MessageType::Unprotected, MessageType::TamperResistant];
        for &msg_type in &types {
            let mut buf = Vec::new();
            let mut writer = Writer::new(std::io::Cursor::new(&mut buf));
            msg_type.to_writer(&mut writer, ()).unwrap();
            writer.finalize().unwrap();

            let mut reader = Reader::new(std::io::Cursor::new(&buf));
            let read_type = MessageType::from_reader_with_ctx(&mut reader, ()).unwrap();
            assert_eq!(msg_type, read_type);
        }
    }

    #[test]
    fn test_real_packet() {
        let data: [u8; 25] = [
            0xce, 0x00, 0xff, 0xff, 0xff, 0xff, 0x00, 0x01, 0x03, 0x00, 0x00, 0x00,
            0xa7, 0x00, 0xa6, 0x8f, 0x80, 0x94, 0x00, 0x00, 0x00, 0x00, 0x72, 0x91,
            0x55,
        ];
        let mut reader = Reader::new(std::io::Cursor::new(&data));
        let frame = PacketFrame::<ReplicationPacket>::from_reader_with_ctx(&mut reader, data.len()).unwrap();

        // Frame header
        assert_eq!(frame.magic_number, MAGIC_NUMBER);
        assert_eq!(frame.message_type, MessageType::Unprotected);
        assert_eq!(frame.checksum, 0xffffffff);
        assert_eq!(frame.packet_index, 0);
        assert_eq!(frame.packet_count, 1);

        // Inner replication packet
        let replication = &frame.inner;
        assert_eq!(replication.packet_id, PacketId::Rpc);
        assert_eq!(replication.index, 0);

        // Extract the RPC packet
        let Packet::Rpc(rpc) = &replication.data else {
            panic!("Expected Rpc packet");
        };

        // Verify RPC fields
        assert_eq!(*rpc.network_id, 0, "Static event has network_id 0");
        assert_eq!(*rpc.blocked_by_network_id, 0);
        assert_eq!(rpc.event_id, 167);

        // Parse the payload
        match rpc.parse_static_event_embedded().unwrap() {
            StaticEventPayload::MySessionComponentMatch_RecieveTimeSync(time_sync) => {
                assert_eq!(*time_sync.sync_time_seconds, 3150324.8);
                assert_eq!(*time_sync.time_left_seconds, 0.0);
            }
            other => panic!("Expected RecieveTimeSync payload, got {:?}", other),
        }
    }

    #[test]
    fn test_real_packet_2() {
        let data: [u8; 76] = [
            0xce, 0x01, 0xa0, 0x8b, 0x06, 0x87, 0x00, 0x01, 0x07, 0x00, 0x72, 0x03,
            0x00, 0x00, 0x00, 0xf4, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x74,
            0x54, 0x55, 0xfb, 0x00, 0xd4, 0x57, 0x03, 0x11, 0x81, 0xbe, 0xa0, 0x14,
            0x01, 0x48, 0xe1, 0x7a, 0x14, 0xf8, 0xe6, 0x46, 0x04, 0x01, 0x00, 0x00,
            0x00, 0x00, 0x00, 0xc0, 0xff, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xc0,
            0xff, 0x02, 0x00, 0x00, 0xfe, 0xc0, 0x64, 0x0a, 0x02, 0xf9, 0x5c, 0xad,
            0xfe, 0xe4, 0x22, 0x03,
        ];

        let mut reader = Reader::new(std::io::Cursor::new(&data));
        let frame = PacketFrame::<ReplicationPacket>::from_reader_with_ctx(&mut reader, data.len()).unwrap();

        // Frame header
        assert_eq!(frame.magic_number, MAGIC_NUMBER);
        assert_eq!(frame.message_type, MessageType::TamperResistant);
        assert_eq!(frame.checksum, 2265353120);
        assert_eq!(frame.packet_index, 0);
        assert_eq!(frame.packet_count, 1);

        // Inner replication packet
        let replication = &frame.inner;
        assert_eq!(replication.packet_id, PacketId::ServerStateSync);
        assert_eq!(replication.index, 0);

        // Extract the ServerStateSync packet
        let Packet::ServerStateSync(packet) = &replication.data else {
            panic!("Expected ServerStateSync packet");
        };

        // Verify ServerStateSync fields
        assert!(!packet.is_streaming);
        assert_eq!(*packet.packet_id, 185);
        
        let stats = packet.statistics.as_ref().expect("Expected statistics");
        assert_eq!(*stats.duplicates, 0);
        assert_eq!(*stats.out_of_order, 0);
        assert_eq!(*stats.drops, 0);
        assert_eq!(*stats.tampered, 0);
        assert_eq!(*stats.outgoing_data, 701);
        assert_eq!(*stats.incoming_data, 0);
        assert_eq!(*stats.pending_packet_count, 0);
        
        assert_eq!(*packet.server_timestamp, 290415.505);
        assert_eq!(*packet.last_client_timestamp, -1.0);
        assert_eq!(*packet.last_client_realtime, -1.0);
        assert_eq!(*packet.server_simulation_ratio, 1.0);
        assert!(packet.state_groups.entries.is_empty());
    }
}
