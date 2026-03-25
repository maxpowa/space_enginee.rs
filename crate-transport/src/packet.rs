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
    #[deku(magic = b"\xCE")]
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
}
