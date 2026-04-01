//! Replication layer for Space Engineers network protocol.
//!
//! This module defines the replication packet structure and the various
//! packet types used for client-server communication.

use deku::bitvec::{BitField, BitSlice, Msb0};
use deku::prelude::*;
use space_engineers_compat::math::{Quaternion, Vector3D};
use space_engineers_compat::{
    BitAligned, BitBool, Nullable, PacketCompressedXmlObject, VarBytes, VarString, Varint,
};
use space_engineers_sys::types::{MyObjectBuilder_Player, MyObjectBuilder_World};

use crate::packet::PacketFrame;
use crate::rpc::RawRpcPacket;

/// Network ID is a VLQ-encoded identifier for replication objects.
pub type NetworkId = Varint<u32>;

/// Packet identifier byte marking the packet type.
#[derive(Clone, Copy, Debug, PartialEq, deku::DekuRead, deku::DekuWrite)]
#[repr(u8)]
#[deku(id_type = "u8")]
pub enum PacketId {
    Unknown = 0,
    Flush = 2,
    Rpc = 3,
    ReplicationCreate = 4,
    ReplicationDestroy = 5,
    ServerData = 6,
    ServerStateSync = 7,
    ClientReady = 8,
    ClientUpdate = 9,
    ReplicationReady = 10,
    ReplicationStreamBegin = 11,
    JoinResult = 12,
    WorldData = 13,
    ClientConnected = 14,
    ClientAcks = 17,
    ReplicationIslandDone = 18,
    ReplicationRequest = 19,
    World = 20,
    PlayerData = 21,
}

/// Top-level replication packet wrapper.
///
/// Every packet sent over the network is wrapped in this structure.
#[derive(Debug, PartialEq, deku::DekuRead, deku::DekuWrite)]
#[deku(ctx = "size: usize")]
pub struct ReplicationPacket {
    /// Packet type identifier
    pub packet_id: PacketId,
    /// Sequence index for ordering
    pub index: u8,
    /// Packet payload (type depends on packet_id)
    #[deku(ctx = "*packet_id, size - 2")]
    pub data: Packet,
    /// Sender endpoint (not serialized, set after parsing)
    #[deku(skip, default = "0")]
    pub sender: u64,
}

/// Replication create packet header.
///
/// Sent by the server when a new replicable object enters the client's scope.
/// After this header, the replicable writes its own type-specific data via OnSave().
#[derive(Debug, PartialEq, deku::DekuRead, deku::DekuWrite)]
pub struct ReplicationCreatePacket {
    pub type_id: Varint<u32>,
    pub network_id: NetworkId,
    pub parent_id: NetworkId,
    #[deku(update = "self.state_group_ids.len() as u8")]
    pub state_group_count: u8,
    #[deku(count = "state_group_count")]
    pub state_group_ids: Vec<NetworkId>,
    /// Raw OnSave data (type-specific, not parsed)
    #[deku(read_all)]
    pub data: Vec<u8>,
}

/// Replication destroy packet.
#[derive(Debug, PartialEq, deku::DekuRead, deku::DekuWrite)]
pub struct ReplicationDestroyPacket {
    /// Network ID of the destroyed replicable
    pub network_id: NetworkId,
}

/// Replication island done packet.
#[derive(Debug, PartialEq, deku::DekuRead, deku::DekuWrite)]
pub struct ReplicationIslandDonePacket {
    pub index: u8,
    #[deku(update = "BitAligned(self.entities.len() as i32)")]
    pub entity_count: BitAligned<i32>,
    #[deku(count = "entity_count.0 as usize")]
    pub entities: Vec<IslandEntity>,
}

#[derive(Debug, PartialEq, deku::DekuRead, deku::DekuWrite)]
pub struct IslandEntity {
    pub entity_id: BitAligned<i64>,
    pub position: Vector3D,
    pub rotation: Quaternion,
}

#[derive(Debug, PartialEq, DekuRead, DekuWrite)]
pub struct ReplicationStreamBeginPacket {
    pub type_id: Varint<u32>,
    pub network_id: NetworkId,
    pub parent_id: NetworkId,
    #[deku(update = "self.state_group_ids.len() as u8")]
    pub state_group_count: u8,
    #[deku(count = "state_group_count")]
    pub state_group_ids: Vec<NetworkId>,
}

#[derive(Debug, PartialEq, DekuRead, DekuWrite)]
pub struct ReplicationRequestPacket {
    pub entity_id: BitAligned<i64>,
    pub add: BitBool,
    pub has_medical_room: BitBool,
    pub has_interacted_entity: BitBool,
    #[deku(cond = "add.get()")]
    pub layer: Option<BitAligned<u8>>,
    #[deku(cond = "has_medical_room.get()")]
    pub medical_room_id: Option<BitAligned<i64>>,
    #[deku(cond = "has_interacted_entity.get()")]
    pub interacted_entity_id: Option<BitAligned<i64>>,
}

#[derive(Debug, PartialEq, DekuRead, DekuWrite)]
pub struct ReplicationReadyPacket {
    pub network_id: NetworkId, // Varint<u32>
    pub loaded: BitBool,
    pub terminator: BitAligned<u16>, // always 51385
}

/// Game mode indicator.
#[derive(Clone, Copy, Debug, Default, PartialEq, deku::DekuRead, deku::DekuWrite)]
#[repr(u8)]
#[deku(id_type = "u8", bits = 1, bit_order = "lsb")]
pub enum GameMode {
    #[default]
    Creative = 0,
    Survival = 1,
}

/// Context kind for client state.
#[derive(Clone, Copy, Debug, Default, PartialEq, deku::DekuRead, deku::DekuWrite)]
#[repr(u8)]
#[deku(id_type = "u8", bits = 2, bit_order = "lsb")]
pub enum ContextKind {
    #[default]
    None = 0,
    Terminal = 1,
    Inventory = 2,
    Production = 3,
}

/// Client state information sent with client update packets.
#[derive(Debug, PartialEq, deku::DekuRead, deku::DekuWrite)]
pub struct ClientState {
    pub has_controlled_entity: BitBool,

    #[deku(
        cond = "!has_controlled_entity.get()",
        default = "BitBool::from(false)"
    )]
    pub has_spectator_camera_position: BitBool,

    #[deku(cond = "has_spectator_camera_position.get()")]
    pub position: space_engineers_sys::math::Vector3D,

    #[deku(cond = "has_controlled_entity.get()")]
    pub controlled_entity_id: BitAligned<i64>,

    #[deku(cond = "has_controlled_entity.get()")]
    pub has_control: BitBool,

    pub magic: BitAligned<i16>,

    #[deku(cond = "has_controlled_entity.get()")]
    pub context_by_page: ContextKind,

    #[deku(cond = "has_controlled_entity.get() && *context_by_page != ContextKind::None")]
    pub interacted_entity_id: BitAligned<i64>,
}

/// Client ready notification packet.
#[derive(Debug, PartialEq, deku::DekuRead, deku::DekuWrite)]
pub struct ClientReadyPacket {
    pub force_playout_delay_buffer: BitBool,
    pub use_playout_delay_buffer_for_character: BitBool,
    pub use_playout_delay_buffer_for_jetpack: BitBool,
    pub use_playout_delay_buffer_for_grids: BitBool,
}

/// Client update packet with timestamp and state.
#[derive(Debug, PartialEq, deku::DekuRead, deku::DekuWrite)]
pub struct ClientUpdatePacket {
    pub packet_id: BitAligned<u8>,
    pub server_timestamp: BitAligned<f64>,
    pub client_timestamp: BitAligned<f64>,
    pub client_state: ClientState,
}

/// Server data packet containing event hash table for synchronization.
#[derive(Debug, PartialEq, deku::DekuRead, deku::DekuWrite)]
pub struct ServerDataPacket {
    #[deku(update = "Varint(self.hash_table.len() as u32)")]
    pub count: Varint<u32>,
    #[deku(count = "count.0")]
    pub hash_table: Vec<i32>,
}

/// Statistics state for server monitoring.
#[derive(Debug, PartialEq, Default, deku::DekuRead, deku::DekuWrite)]
pub struct StatisticsState {
    pub duplicates: BitAligned<u8>,
    pub out_of_order: BitAligned<u8>,
    pub drops: BitAligned<u8>,
    pub tampered: BitAligned<u8>,
    pub outgoing_data: BitAligned<i32>,
    pub incoming_data: BitAligned<i32>,
    pub timeout_remaining: BitAligned<f32>,
    pub pending_packet_count: BitAligned<u8>,
    pub gc_memory: BitAligned<f32>,
    pub process_memory: BitAligned<f32>,
    pub playout_delay_buffer_size: BitAligned<u8>,
}

/// A single state group entry from the server state sync packet
#[derive(Debug, PartialEq)]
pub struct StateGroupEntry {
    /// Network ID of the state group
    pub network_id: NetworkId,
    /// Size of the state group data in bits
    pub size_bits: i32,
    /// Raw state group data (we don't parse the internals)
    pub data: Vec<u8>,
}

/// Container for state group entries with custom parsing
#[derive(Debug, PartialEq, Default)]
pub struct StateGroups {
    pub entries: Vec<StateGroupEntry>,
}

impl<'a> DekuReader<'a, bool> for StateGroups {
    fn from_reader_with_ctx<R: std::io::Read + std::io::Seek>(
        reader: &mut Reader<R>,
        is_streaming: bool,
    ) -> Result<Self, DekuError> {
        Self::read_entries(reader, is_streaming).map(|(groups, _)| groups)
    }
}

impl DekuWriter<bool> for StateGroups {
    fn to_writer<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut Writer<W>,
        is_streaming: bool,
    ) -> Result<(), DekuError> {
        use deku::ctx::Order;

        const STATE_GROUP_TERMINATOR: u16 = 51385; // 0xC8B9
        const FINAL_TERMINATOR: u16 = 0;

        for entry in &self.entries {
            // Write state group terminator (51385)
            let term_bytes = STATE_GROUP_TERMINATOR.to_le_bytes();
            let bits = BitSlice::<u8, Msb0>::from_slice(&term_bytes);
            writer.write_bits_order(bits, Order::Lsb0)?;

            // Write network_id as Varint
            entry.network_id.to_writer(writer, ())?;

            // Write size_bits
            if is_streaming {
                // i32 for streaming
                let size_bytes = (entry.size_bits as i32).to_le_bytes();
                let bits = BitSlice::<u8, Msb0>::from_slice(&size_bytes);
                writer.write_bits_order(bits, Order::Lsb0)?;
            } else {
                // i16 for non-streaming
                let size_bytes = (entry.size_bits as i16).to_le_bytes();
                let bits = BitSlice::<u8, Msb0>::from_slice(&size_bytes);
                writer.write_bits_order(bits, Order::Lsb0)?;
            }

            // Write data bytes
            let data_bits = BitSlice::<u8, Msb0>::from_slice(&entry.data);
            writer.write_bits_order(data_bits, Order::Lsb0)?;
        }

        // Write final terminator (0x0000)
        let term_bytes = FINAL_TERMINATOR.to_le_bytes();
        let bits = BitSlice::<u8, Msb0>::from_slice(&term_bytes);
        writer.write_bits_order(bits, Order::Lsb0)?;

        Ok(())
    }
}

impl StateGroups {
    /// Read state groups until we hit a non-51385 terminator or run out of data
    /// C#: while (BytePosition + 2 < ByteLength) { CheckTerminator(); ... }
    fn read_entries<R: std::io::Read + std::io::Seek>(
        reader: &mut Reader<R>,
        is_streaming: bool,
    ) -> Result<(Self, u16), DekuError> {
        use deku::ctx::Order;

        let mut entries = Vec::new();

        // Read all remaining bytes first so we know the length
        let mut remaining_bytes = Vec::new();
        loop {
            match reader.read_bits(8, Order::Lsb0) {
                Ok(Some(bits)) => remaining_bytes.push(bits.load_le::<u8>()),
                _ => break,
            }
        }

        let total_bytes = remaining_bytes.len();
        let mut pos = 0;

        // C#: while (BytePosition + 2 < ByteLength)
        while pos + 2 < total_bytes {
            // Read terminator (2 bytes)
            let terminator =
                (remaining_bytes[pos] as u16) | ((remaining_bytes[pos + 1] as u16) << 8);
            pos += 2;

            // If not 51385, this is the final terminator - we're done
            if terminator != 51385 {
                return Ok((StateGroups { entries }, terminator));
            }

            // Read NetworkId (Varint encoded) - read bytes until continuation bit is 0
            let mut network_id: u32 = 0;
            let mut shift = 0;
            for _ in 0..5 {
                if pos >= total_bytes {
                    return Ok((StateGroups { entries }, 0));
                }
                let byte = remaining_bytes[pos];
                pos += 1;
                network_id |= ((byte & 0x7F) as u32) << shift;
                if (byte & 0x80) == 0 {
                    break;
                }
                shift += 7;
            }

            // Read size in bits
            if pos + 2 > total_bytes {
                return Ok((StateGroups { entries }, 0));
            }

            let size_bits: i32 = if is_streaming {
                if pos + 4 > total_bytes {
                    return Ok((StateGroups { entries }, 0));
                }
                let val = (remaining_bytes[pos] as i32)
                    | ((remaining_bytes[pos + 1] as i32) << 8)
                    | ((remaining_bytes[pos + 2] as i32) << 16)
                    | ((remaining_bytes[pos + 3] as i32) << 24);
                pos += 4;
                val
            } else {
                let val = (remaining_bytes[pos] as i16) | ((remaining_bytes[pos + 1] as i16) << 8);
                pos += 2;
                val as i32
            };

            // Read state group data (size_bits bits = ceil(size_bits/8) bytes)
            let data_bytes = ((size_bits + 7) / 8) as usize;
            if pos + data_bytes > total_bytes {
                return Ok((StateGroups { entries }, 0));
            }

            let data = remaining_bytes[pos..pos + data_bytes].to_vec();
            pos += data_bytes;

            entries.push(StateGroupEntry {
                network_id: Varint(network_id),
                size_bits,
                data,
            });
        }

        // Read final terminator if we have exactly 2 bytes left
        let final_terminator = if pos + 2 <= total_bytes {
            (remaining_bytes[pos] as u16) | ((remaining_bytes[pos + 1] as u16) << 8)
        } else {
            0
        };

        Ok((StateGroups { entries }, final_terminator))
    }
}

#[derive(Debug, PartialEq, deku::DekuRead, deku::DekuWrite)]
pub struct ServerStatePacket {
    // See VRage.Network.MyReplicationClient.OnServerStateSync
    pub is_streaming: BitBool,
    pub packet_id: BitAligned<u8>,
    pub statistics: Nullable<StatisticsState>,
    pub server_timestamp: BitAligned<f64>,
    pub last_client_timestamp: BitAligned<f64>,
    pub last_client_realtime: BitAligned<f64>,
    // Note: m_callback.ReadCustomState() reads these in SE
    pub server_simulation_ratio: BitAligned<f32>,
    pub server_cpu_load: BitAligned<f32>,
    pub server_thread_load: BitAligned<f32>,
    #[deku(ctx = "is_streaming.get()")]
    pub state_groups: StateGroups,
}

/// World data packet containing server/world metadata.
#[derive(Debug, PartialEq, deku::DekuRead, deku::DekuWrite)]
pub struct WorldDataPacket {
    pub app_version: BitAligned<i32>,
    pub assembler_multiplier: BitAligned<f32>,
    pub blocks_inventory_multiplier: BitAligned<f32>,
    pub data_hash: Nullable<VarString>,
    pub game_mode: GameMode,
    pub grinder_multiplier: BitAligned<f32>,
    pub host_name: Nullable<VarString>,
    pub inventory_multiplier: BitAligned<f32>,
    pub members_limit: BitAligned<i32>,
    pub refinery_multiplier: BitAligned<f32>,
    pub server_analytics_id: Nullable<VarString>,
    pub server_password_salt: Nullable<VarString>,
    pub welder_multiplier: BitAligned<f32>,
    pub world_name: Nullable<VarString>,
    pub world_size: BitAligned<u64>,
}

/// Client connected packet for connection handshake.
#[derive(Clone, Debug, PartialEq, deku::DekuRead, deku::DekuWrite)]
pub struct ClientConnectedPacket {
    pub client_id: BitAligned<u64>,
    pub experimental_mode: BitBool,
    pub is_admin: BitBool,
    pub is_profiling: BitBool,
    pub join: BitBool,
    pub name: Nullable<VarString>,
    pub service_name: Nullable<VarString>,
    pub token: Nullable<VarBytes>,
}

/// Client acknowledgment packet.
#[derive(Clone, Debug, PartialEq, deku::DekuRead, deku::DekuWrite)]
pub struct ClientAckPacket {
    pub last_state_sync_packet_id: BitAligned<u8>,
    pub received_streaming_packet: BitBool,
    pub last_streaming_packet_id: BitAligned<u8>,
    #[deku(update = "BitAligned(self.ack_packets.len() as u8)")]
    pub ack_packet_count: BitAligned<u8>,
    #[deku(count = "ack_packet_count.0 as usize")]
    pub ack_packets: Vec<BitAligned<u8>>,
}

/// Join result codes.
#[derive(Clone, Copy, Debug, Default, PartialEq, deku::DekuRead, deku::DekuWrite)]
#[repr(u8)]
#[deku(id_type = "u8", bits = 5, bit_order = "lsb")]
pub enum JoinResult {
    #[default]
    Ok = 0,
    AlreadyJoined,
    TicketInvalid,
    SteamServersOffline,
    NotInGroup,
    GroupIdInvalid,
    ServerFull,
    BannedByAdmins,
    KickedRecently,
    TicketCanceled,
    TicketAlreadyUsed,
    LoggedInElseWhere,
    NoLicenseOrExpired,
    UserNotConnected,
    VacBanned,
    VacCheckTimedOut,
    PasswordRequired,
    WrongPassword,
    ExperimentalMode,
    ProfilingNotAllowed,
    FamilySharing,
    NotFound,
    IncorrectTime,
}

/// Join result packet.
#[derive(Debug, PartialEq, deku::DekuRead, deku::DekuWrite)]
pub struct JoinResultPacket {
    pub admin_id: BitAligned<u64>,
    pub join_result: JoinResult,
    pub server_experimental_mode: BitBool,
}

#[derive(Debug, PartialEq, deku::DekuRead, deku::DekuWrite)]
pub struct WorldPacket {
    // Contains the entire world data in a gzipped byte array.
    pub data: PacketCompressedXmlObject<MyObjectBuilder_World>,
}

#[derive(Debug, PartialEq, DekuRead, DekuWrite)]
pub struct PlayerDataPacket {
    pub client_steam_id: BitAligned<u64>,
    pub player_serial_id: BitAligned<i32>,
    pub new_identity: BitBool,
    pub player_builder_data: MyObjectBuilder_Player,
}

/// Packet payload enum dispatched by packet ID.
#[derive(Debug, PartialEq, deku::DekuRead, deku::DekuWrite)]
#[deku(ctx = "identifier: PacketId, size: usize", id = "identifier")]
pub enum Packet {
    #[deku(id = "PacketId::Flush")]
    Flush(()),
    #[deku(id = "PacketId::Rpc")]
    Rpc(RawRpcPacket),
    #[deku(id = "PacketId::ReplicationCreate")]
    ReplicationCreate(ReplicationCreatePacket),
    #[deku(id = "PacketId::ReplicationDestroy")]
    ReplicationDestroy(ReplicationDestroyPacket),
    #[deku(id = "PacketId::ServerData")]
    ServerData(ServerDataPacket),
    #[deku(id = "PacketId::ServerStateSync")]
    ServerStateSync(ServerStatePacket),
    #[deku(id = "PacketId::ClientReady")]
    ClientReady(ClientReadyPacket),
    #[deku(id = "PacketId::ClientUpdate")]
    ClientUpdate(ClientUpdatePacket),
    #[deku(id = "PacketId::ReplicationReady")]
    ReplicationReady(ReplicationReadyPacket),
    #[deku(id = "PacketId::ReplicationStreamBegin")]
    ReplicationStreamBegin(ReplicationStreamBeginPacket),
    #[deku(id = "PacketId::JoinResult")]
    JoinResult(JoinResultPacket),
    #[deku(id = "PacketId::WorldData")]
    WorldData(WorldDataPacket),
    #[deku(id = "PacketId::ClientConnected")]
    ClientConnected(ClientConnectedPacket),
    #[deku(id = "PacketId::ClientAcks")]
    ClientAcks(ClientAckPacket),
    #[deku(id = "PacketId::ReplicationIslandDone")]
    ReplicationIslandDone(ReplicationIslandDonePacket),
    #[deku(id = "PacketId::ReplicationRequest")]
    ReplicationRequest(ReplicationRequestPacket),
    #[deku(id = "PacketId::World")]
    World(WorldPacket),
    #[deku(id = "PacketId::PlayerData")]
    PlayerData(PlayerDataPacket),
    #[deku(id_pat = "_")]
    Unknown,
}

impl From<ReplicationPacket> for PacketFrame<ReplicationPacket> {
    fn from(data: ReplicationPacket) -> Self {
        Self::unprotected(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_packet_id_round_trip() {
        let ids = [PacketId::Flush, PacketId::Rpc, PacketId::ClientConnected];
        for &id in &ids {
            let mut buf = Vec::new();
            let mut writer = Writer::new(std::io::Cursor::new(&mut buf));
            id.to_writer(&mut writer, ()).unwrap();
            writer.finalize().unwrap();

            let mut reader = Reader::new(std::io::Cursor::new(&buf));
            let read_id = PacketId::from_reader_with_ctx(&mut reader, ()).unwrap();
            assert_eq!(id, read_id);
        }
    }

    #[test]
    pub fn world_test_packet_parse() {
        let data = include_bytes!("../test_data/world_packet.bin");

        // Treat content as raw binary data (including the header frame)
        let mut reader = Reader::new(Cursor::new(data.as_slice()));
        let frame = PacketFrame::<ReplicationPacket>::from_reader_with_ctx(&mut reader, data.len())
            .unwrap();
        let replication_packet = frame.inner;

        // Now you can work with the replication_packet
        assert_eq!(replication_packet.packet_id, PacketId::World);
        if let Packet::World(world) = replication_packet.data {
            assert_eq!(world.data.0.checkpoint.app_version, 1208015);
        } else {
            panic!("Expected WorldData packet");
        }
    }

    #[test]
    pub fn server_state_sync_parse() {
        let data: [u8; 194] = [
            0xce, 0x01, 0x80, 0xe0, 0x5d, 0xca, 0x00, 0x01, 0x07, 0x00, 0x7e, 0xa8, 0x13, 0xd0,
            0x5c, 0x34, 0x1a, 0x40, 0x06, 0xfd, 0x65, 0xf7, 0x4e, 0x32, 0x1a, 0x40, 0x06, 0xb1,
            0xb6, 0xe2, 0x33, 0xb2, 0x4f, 0xfb, 0x06, 0x01, 0x00, 0x00, 0xfe, 0x00, 0x91, 0x5e,
            0x00, 0xd5, 0x12, 0x72, 0xfc, 0xe4, 0x22, 0x9b, 0x1b, 0x40, 0x11, 0x00, 0x94, 0x4f,
            0x55, 0xeb, 0x44, 0xc1, 0xed, 0x0b, 0xd4, 0x41, 0x2b, 0x67, 0xb3, 0xdf, 0xa4, 0x0c,
            0xc4, 0x71, 0xdc, 0x33, 0xe9, 0xc0, 0x18, 0x0b, 0x0c, 0x00, 0x00, 0x00, 0x60, 0x66,
            0x66, 0xf6, 0x03, 0x00, 0x00, 0x00, 0x80, 0x8e, 0xd0, 0xea, 0x7b, 0x04, 0x43, 0xf5,
            0x13, 0x71, 0xb8, 0xe9, 0xc3, 0x65, 0x71, 0xea, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x80, 0xfe, 0xfe, 0xfe, 0xce, 0x11, 0x5a, 0x7d, 0x8b, 0x60, 0xa8, 0x7e,
            0x20, 0x0e, 0x37, 0x7d, 0xb6, 0x2c, 0x4e, 0x7d, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xe4, 0x22, 0x03,
        ];

        // Treat content as raw binary data (including the header frame)
        let mut reader = Reader::new(Cursor::new(data));
        let frame = PacketFrame::<ReplicationPacket>::from_reader_with_ctx(&mut reader, data.len())
            .unwrap();
        let replication_packet = frame.inner;

        // check if replication_packet was parsed correctly
        assert_eq!(replication_packet.packet_id, PacketId::ServerStateSync);
        if let Packet::ServerStateSync(server_state_sync) = replication_packet.data {
            // Byte 10 = 0x7e = 0b01111110, bit 0 (LSB) = 0 -> is_streaming = false
            assert_eq!(server_state_sync.is_streaming, false);
            // packet_id is next 8 bits: 0x7e >> 1 = 0x3F = 63
            assert_eq!(server_state_sync.packet_id.0, 63);
            // statistics has_value bit = 0, so None
            assert!(server_state_sync.statistics.is_none());
            // Timestamps look plausible (milliseconds)
            assert_eq!(server_state_sync.server_timestamp.0, 67216197.8008f64);
            assert_eq!(server_state_sync.last_client_timestamp.0, 67216164.9354f64);
            // Custom state values
            println!(
                "server_simulation_ratio: {}",
                server_state_sync.server_simulation_ratio.0
            );
            println!("server_cpu_load: {}", server_state_sync.server_cpu_load.0);
            println!(
                "server_thread_load: {}",
                server_state_sync.server_thread_load.0
            );
            // Print state groups for debugging
            println!(
                "Number of state groups: {}",
                server_state_sync.state_groups.entries.len()
            );
            for (i, entry) in server_state_sync.state_groups.entries.iter().enumerate() {
                println!(
                    "  State group {}: network_id={:?}, size_bits={}, data_len={}",
                    i,
                    entry.network_id,
                    entry.size_bits,
                    entry.data.len()
                );
            }
            // println!("Final terminator: 0x{:04X}", server_state_sync.terminator);
            // Note: This packet appears to be truncated - the state group data extends beyond
            // the captured bytes, so we don't get a complete state group or valid final terminator.
            // The terminator will be 0 because we ran out of data while reading state group content.
            // In a complete packet, we would expect to either see no state groups (with terminator != 51385)
            // or complete state groups followed by a non-51385 terminator value.
        } else {
            panic!("Expected ServerStateSync packet");
        }
    }

    #[test]
    pub fn client_acks_packet_parse() {
        let data: [u8; 17] = [
            0xce, 0x00, 0xff, 0xff, 0xff, 0xff, 0x00, 0x01, 0x11, 0x00, 0x95, 0x00, 0x02, 0x2a,
            0x73, 0x91, 0x01,
        ];

        let mut reader = Reader::new(Cursor::new(data));
        let frame = PacketFrame::<ReplicationPacket>::from_reader_with_ctx(&mut reader, data.len())
            .unwrap();
        let replication_packet = frame.inner;

        assert_eq!(replication_packet.packet_id, PacketId::ClientAcks);
        if let Packet::ClientAcks(client_acks) = replication_packet.data {
            // Bit-level decoding with BS<u8> (8 bits each, bit-aligned):
            assert_eq!(client_acks.last_state_sync_packet_id, 149.into());
            assert_eq!(client_acks.received_streaming_packet, false);
            assert_eq!(client_acks.last_streaming_packet_id, 0.into());
            assert_eq!(client_acks.ack_packet_count, 1.into());
            assert_eq!(client_acks.ack_packets, vec![149.into()]);
        } else {
            panic!("Expected ClientAcks packet");
        }
    }

    #[test]
    pub fn rpc_packet_parse() {
        // This is a real RPC packet sample with a bit-shifted terminator.
        let data: [u8; 35] = [
            0xce, 0x00, 0xff, 0xff, 0xff, 0xff, 0x00, 0x01, 0x03, 0x00, 0xdb, 0x49, 0x00, 0x00,
            0x00, 0x02, 0x44, 0xb5, 0xb7, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x1c, 0x42, 0x86, 0xc2, 0xe5, 0x22, 0x07,
        ];

        let mut reader = Reader::new(Cursor::new(data));
        let frame = PacketFrame::<ReplicationPacket>::from_reader_with_ctx(&mut reader, data.len())
            .unwrap();
        let replication_packet = frame.inner;

        assert_eq!(replication_packet.packet_id, PacketId::Rpc);
        if let Packet::Rpc(rpc) = replication_packet.data {
            assert_eq!(rpc.network_id.0, 9435);
            assert_eq!(rpc.blocked_by_network_id.0, 0);
            assert_eq!(rpc.event_id, 0);
            assert!(rpc.position.is_none());
            assert_eq!(rpc.payload.len(), 18);
        } else {
            panic!("Expected Rpc packet");
        }
    }
}
