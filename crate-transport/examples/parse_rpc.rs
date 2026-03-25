//! Example: Parsing RPC packets from Space Engineers network data.
//!
//! This example demonstrates how to parse different RPC types,
//! including both static (global) events and instance (object-specific) events.
//!
//! Run with: `cargo run --example parse_rpc`

use space_engineers_compat::{Nullable, Varint};
use space_engineers_transport::{
    // RPC types
    RawRpcPacket, RpcError, StaticEventPayload, StaticRpcPacket,
    // Version schema (for multi-version support)
    Version,
    // Type identities
    ReplicatedType,
};

// Instance events are in the protocol module
use space_engineers_transport::protocol::{
    // Static event visitor and payloads
    OnChatMessageReceived_ServerPayload, OnPlayerCreatedPayload, StaticEventVisitor,
    // Instance events
    MyCubeGridInstanceEvent, MyCubeGridInstanceEventPayload,
    MyCubeGrid_MergeGrid_MergeClientPayload, MyCubeGridInstanceEventVisitor,
};

fn main() {
    println!("Space Engineers RPC Parsing Examples");
    println!("====================================\n");

    println!("Embedded schema version: {}", Version::embedded_version());
    println!();

    // Run the mock server/client simulation
    run_mock_simulation();
}

// =============================================================================
// Mock Server/Client Simulation
// =============================================================================

/// Simulates a game server sending RPC packets to a client.
fn run_mock_simulation() {
    println!("=== Mock Server/Client Simulation ===\n");

    // Create client state
    let mut client = GameClient::new();

    // Server sends a series of events
    let server = MockServer::new();

    // 1. Server creates a grid (replication create)
    println!("Server: Creating grid with network_id=1...");
    client.on_replication_create(1, ReplicatedType::MyCubeGrid.type_hash());
    // Name the grid
    if let Some(grid) = client.state.grids.get_mut(&1) {
        grid.name = "My First Ship".to_string();
    }
    println!();

    // 2. Server creates another grid
    println!("Server: Creating grid with network_id=2...");
    client.on_replication_create(2, ReplicatedType::MyCubeGrid.type_hash());
    if let Some(grid) = client.state.grids.get_mut(&2) {
        grid.name = "Mining Drone".to_string();
    }
    println!();

    // 3. Server sends grid changed event to grid 1
    println!("Server: Sending OnGridChangedRPC to grid 1...");
    let grid_event = server.create_grid_changed_packet(1);
    println!("  (packet size: {} bytes)", grid_event.len());
    client.receive_packet(&grid_event);
    println!();

    // 4. Server sends grid changed event to grid 2
    println!("Server: Sending OnGridChangedRPC to grid 2...");
    let grid_event2 = server.create_grid_changed_packet(2);
    client.receive_packet(&grid_event2);
    println!();

    // 5. Send multiple events to grid 1
    println!("Server: Sending more events to grid 1...");
    for _ in 0..3 {
        let packet = server.create_grid_changed_packet(1);
        client.receive_packet(&packet);
    }
    println!();

    // 6. Server destroys grid 2
    println!("Server: Destroying grid 2...");
    client.on_replication_destroy(2);
    println!();

    // Print final state
    println!("=== Final Client State ===");
    println!("Tracked grids: {}", client.state.grids.len());
    for (id, grid) in &client.state.grids {
        println!("  Grid {}: '{}' ({} events received)", id, grid.name, grid.event_count);
    }
}

/// Mock server that creates RPC packets.
struct MockServer {
    protocol_version: Version,
}

impl MockServer {
    fn new() -> Self {
        Self {
            protocol_version: Version::embedded(),
        }
    }

    /// Create an OnGridChangedRPC instance event packet.
    fn create_grid_changed_packet(&self, network_id: u32) -> Vec<u8> {
        // Get the event ID for OnGridChangedRPC for MyCubeGrid
        let event_type = MyCubeGridInstanceEvent::OnGridChangedRPC;
        let type_hash = ReplicatedType::MyCubeGrid.type_hash();
        let event_id = self.protocol_version
            .encode_instance_event_hash(type_hash, event_type.event_hash())
            .expect("OnGridChangedRPC should be in schema");

        // OnGridChangedRPC has no payload
        let packet = RawRpcPacket {
            network_id: Varint(network_id),
            blocked_by_network_id: Varint(0),
            event_id,
            position: Nullable::default(),
            payload: vec![],
        };

        packet.to_bytes().expect("Failed to serialize packet")
    }
}

/// Client that receives and processes RPC packets.
struct GameClient {
    state: ReplicationState,
}

impl GameClient {
    fn new() -> Self {
        Self {
            state: ReplicationState::new(),
        }
    }

    fn on_replication_create(&mut self, network_id: u32, type_hash: i32) {
        self.state.on_replication_create(network_id, type_hash);
        if let Some(type_name) = ReplicatedType::from_hash(type_hash) {
            println!("  Client: Created {:?} with network_id={}", type_name, network_id);
        }
    }

    fn on_replication_destroy(&mut self, network_id: u32) {
        let type_hash = self.state.type_by_network_id.get(&network_id).copied();
        self.state.on_replication_destroy(network_id);
        if let Some(hash) = type_hash {
            if let Some(type_name) = ReplicatedType::from_hash(hash) {
                println!("  Client: Destroyed {:?} with network_id={}", type_name, network_id);
            }
        }
    }

    fn receive_packet(&mut self, data: &[u8]) {
        let packet = match RawRpcPacket::from_bytes(data) {
            Ok(p) => p,
            Err(e) => {
                println!("  Client: Failed to parse packet: {:?}", e);
                return;
            }
        };

        if packet.is_static_event() {
            self.handle_static_event(&packet);
        } else {
            self.handle_instance_event(&packet);
        }
    }

    fn handle_static_event(&mut self, packet: &RawRpcPacket) {
        let payload = match packet.parse_static_event_embedded() {
            Ok(p) => p,
            Err(e) => {
                println!("  Client: Failed to parse static event: {:?}", e);
                return;
            }
        };

        // Use visitor pattern for clean dispatch
        let mut handler = ClientStaticEventHandler;
        payload.accept(&mut handler);
    }

    fn handle_instance_event(&mut self, packet: &RawRpcPacket) {
        let schema = Version::embedded();
        let network_id = packet.network_id.0;

        let type_hash = match self.state.type_by_network_id.get(&network_id) {
            Some(&h) => h,
            None => {
                println!("  Client: Unknown network_id: {}", network_id);
                return;
            }
        };

        // Dispatch based on type
        match ReplicatedType::from_hash(type_hash) {
            Some(ReplicatedType::MyCubeGrid) => {
                let event_hash = match packet.resolve_instance_event_hash(&schema, type_hash) {
                    Ok(h) => h,
                    Err(e) => {
                        println!("  Client: Failed to resolve grid event: {:?}", e);
                        return;
                    }
                };

                if let Some(grid) = self.state.grids.get_mut(&network_id) {
                    if let Some(event) = MyCubeGridInstanceEvent::from_hash(event_hash) {
                        if let Ok(payload) = event.parse_payload(&packet.payload) {
                            let mut handler = ClientGridEventHandler { grid };
                            payload.accept(&mut handler);
                        }
                    }
                }
            }
            _ => {
                println!("  Client: Unhandled type hash: {}", type_hash);
            }
        }
    }
}

/// Visitor for static events - log received events.
struct ClientStaticEventHandler;

impl StaticEventVisitor for ClientStaticEventHandler {
    fn visit_on_chat_message_received_server(&mut self, payload: &OnChatMessageReceived_ServerPayload) {
        println!("  Client: Chat message received: {:?}", payload.msg);
    }

    fn visit_on_player_created(&mut self, payload: &OnPlayerCreatedPayload) {
        println!("  Client: Player created with Steam ID: {}", payload.client_steam_id.0);
    }

    fn visit_unknown(&mut self, event_hash: i32, _payload: &[u8]) {
        println!("  Client: Unknown static event (hash={})", event_hash);
    }
}

/// Visitor for grid instance events.
struct ClientGridEventHandler<'a> {
    grid: &'a mut GridInstance,
}

impl MyCubeGridInstanceEventVisitor for ClientGridEventHandler<'_> {
    fn visit_on_grid_changed_rpc(&mut self) {
        self.grid.event_count += 1;
        println!("  Client: Grid '{}' changed (event #{})", self.grid.name, self.grid.event_count);
    }

    fn visit_merge_grid_merge_client(&mut self, payload: &MyCubeGrid_MergeGrid_MergeClientPayload) {
        self.grid.event_count += 1;
        println!("  Client: Grid '{}' merging: {:?}", self.grid.name, payload);
    }
}

// =============================================================================
// Option 1: Fully-typed static event parsing (single game version)
// =============================================================================

/// Parse static events directly into typed payloads using the embedded schema.
///
/// Use this when you only need to support the game version that the crate
/// was built against. Provides the simplest API with full type safety.
#[allow(dead_code)]
fn handle_static_event_typed(rpc_data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    // Parse directly into typed payload using embedded schema
    // rpc_data should be the RPC packet bytes (after frame parsing)
    let packet = StaticRpcPacket::from_bytes(rpc_data)?;

    match &packet.payload {
        StaticEventPayload::OnChatMessageReceived_Server(msg) => {
            // msg contains the full ChatMsg struct
            println!("Chat message: {:?}", msg.msg);
        }
        StaticEventPayload::ModMessageServerReliable(msg) => {
            // id = mod message channel ID, message = data bytes
            println!(
                "Mod message on channel {}: {} bytes",
                msg.id.0,
                msg.message.0.len()
            );
        }
        StaticEventPayload::OnPlayerCreated(player) => {
            // Contains steam ID, serial ID, player builder
            println!("Player created: steam_id={}", player.client_steam_id.0);
        }
        StaticEventPayload::FactionStateChangeSuccess(faction) => {
            println!("Faction state change: {:?}", faction);
        }
        _ => {
            // There are ~500 static event types - handle the ones you need
            println!("Other static event type");
        }
    }

    Ok(())
}

// =============================================================================
// Option 2: Raw parsing with schema resolution (multi-version support)
// =============================================================================

/// Parse RPC packets in raw form, then resolve using a schema.
///
/// Use this when you need to support multiple game versions. The schema
/// maps version-specific event IDs to stable hashes.
#[allow(dead_code)]
fn handle_raw_rpc(rpc_data: &[u8], state: &mut ReplicationState) -> Result<(), Box<dyn std::error::Error>> {
    // Parse to raw packet first (no schema needed yet)
    let raw_rpc = RawRpcPacket::from_bytes(rpc_data)?;

    if raw_rpc.is_static_event() {
        handle_static_event(&raw_rpc)?;
    } else {
        handle_instance_event_with_state(&raw_rpc, state)?;
    }

    Ok(())
}

/// Parse static events using a schema for version-specific ID resolution.
#[allow(dead_code)]
fn handle_static_event(raw_rpc: &RawRpcPacket) -> Result<(), RpcError> {
    // Use embedded schema (or load a different version's schema)
    let schema = Version::embedded();

    // Parse payload using schema - resolves version-specific ID to stable hash
    let payload = raw_rpc.parse_static_event(&schema)?;

    match payload {
        StaticEventPayload::OnChatMessageReceived_Server(msg) => {
            println!("Chat message: {:?}", msg.msg);
        }
        StaticEventPayload::OnPlayerCreated(player) => {
            println!("Player created: steam_id={}", player.client_steam_id.0);
        }
        StaticEventPayload::FactionStateChangeSuccess(faction) => {
            println!("Faction change: {:?}", faction);
        }
        _ => {
            println!("Other static event");
        }
    }

    Ok(())
}

// =============================================================================
// Instance events - events on specific replicated objects
// =============================================================================

// In a real application, you'd track replicated objects by their network_id.
// Each network_id corresponds to a specific instance of a replicated type.

use std::collections::HashMap;

/// Tracks all replicated objects in the game world.
struct ReplicationState {
    /// Maps network_id -> type hash (set on ReplicationCreate, removed on ReplicationDestroy)
    type_by_network_id: HashMap<u32, i32>,
    
    /// Your actual game objects, indexed by network_id
    grids: HashMap<u32, GridInstance>,
    characters: HashMap<u32, CharacterInstance>,
}

/// A specific grid instance in your application
struct GridInstance {
    #[allow(dead_code)]
    network_id: u32,
    name: String,
    #[allow(dead_code)]
    block_count: usize,
    event_count: usize,
}

/// A specific character instance
#[allow(dead_code)]
struct CharacterInstance {
    network_id: u32,
    steam_id: u64,
}

impl ReplicationState {
    fn new() -> Self {
        Self {
            type_by_network_id: HashMap::new(),
            grids: HashMap::new(),
            characters: HashMap::new(),
        }
    }
    
    /// Called when the server creates a new replicated object
    fn on_replication_create(&mut self, network_id: u32, type_hash: i32) {
        self.type_by_network_id.insert(network_id, type_hash);
        
        // Create the appropriate object instance
        match ReplicatedType::from_hash(type_hash) {
            Some(ReplicatedType::MyCubeGrid) => {
                self.grids.insert(network_id, GridInstance {
                    network_id,
                    name: String::new(),
                    block_count: 0,
                    event_count: 0,
                });
            }
            Some(ReplicatedType::MyCharacter) => {
                self.characters.insert(network_id, CharacterInstance {
                    network_id,
                    steam_id: 0,
                });
            }
            _ => {
                // Handle other types as needed
            }
        }
    }
    
    /// Called when the server destroys a replicated object  
    fn on_replication_destroy(&mut self, network_id: u32) {
        if let Some(type_hash) = self.type_by_network_id.remove(&network_id) {
            match ReplicatedType::from_hash(type_hash) {
                Some(ReplicatedType::MyCubeGrid) => {
                    self.grids.remove(&network_id);
                }
                Some(ReplicatedType::MyCharacter) => {
                    self.characters.remove(&network_id);
                }
                _ => {}
            }
        }
    }
}

/// Dispatch an instance event to the correct object based on network_id.
#[allow(dead_code)]
fn handle_instance_event_with_state(
    raw_rpc: &RawRpcPacket,
    state: &mut ReplicationState,
) -> Result<(), Box<dyn std::error::Error>> {
    let schema = Version::embedded();
    let network_id = raw_rpc.network_id.0;
    
    // Step 1: Look up what type this network_id is
    let type_hash = state.type_by_network_id
        .get(&network_id)
        .copied()
        .ok_or_else(|| format!("Unknown network_id: {}", network_id))?;
    
    // Step 2: Resolve event ID to stable hash
    let event_hash = raw_rpc.resolve_instance_event_hash(&schema, type_hash)?;
    
    // Step 3: Dispatch to the correct instance based on type
    match ReplicatedType::from_hash(type_hash) {
        Some(ReplicatedType::MyCubeGrid) => {
            // Get the specific grid instance
            if let Some(grid) = state.grids.get_mut(&network_id) {
                dispatch_grid_event(grid, event_hash, &raw_rpc.payload)?;
            }
        }
        Some(ReplicatedType::MyCharacter) => {
            if let Some(character) = state.characters.get_mut(&network_id) {
                dispatch_character_event(character, event_hash, &raw_rpc.payload)?;
            }
        }
        _ => {
            println!("Unhandled type hash: {}", type_hash);
        }
    }
    
    Ok(())
}

/// Handle events for a specific grid instance
fn dispatch_grid_event(
    grid: &mut GridInstance,
    event_hash: i32,
    payload_bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let event = MyCubeGridInstanceEvent::from_hash(event_hash)
        .ok_or_else(|| format!("Unknown grid event hash: {}", event_hash))?;
    
    let payload = event.parse_payload(payload_bytes)?;
    
    // Now we have both the specific grid instance AND the parsed event
    match payload {
        MyCubeGridInstanceEventPayload::MergeGrid_MergeClient(merge) => {
            println!(
                "Grid '{}' (id={}) merging: {:?}",
                grid.name, grid.network_id, merge
            );
            // Update the grid instance as needed
        }
        MyCubeGridInstanceEventPayload::OnGridChangedRPC => {
            println!("Grid '{}' (id={}) changed", grid.name, grid.network_id);
        }
        MyCubeGridInstanceEventPayload::CreateSplit_Implementation(split) => {
            println!(
                "Grid '{}' (id={}) splitting: {:?}",
                grid.name, grid.network_id, split
            );
        }
        _ => {
            println!("Grid '{}' received: {:?}", grid.name, event);
        }
    }
    
    Ok(())
}

fn dispatch_character_event(
    _character: &mut CharacterInstance,
    _event_hash: i32,
    _payload_bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    // Similar pattern for character events...
    Ok(())
}

// -----------------------------------------------------------------------------
// Using visitors with instance tracking
// -----------------------------------------------------------------------------

/// A visitor that updates a specific grid instance based on events.
struct GridEventHandler<'a> {
    grid: &'a mut GridInstance,
}

impl MyCubeGridInstanceEventVisitor for GridEventHandler<'_> {
    fn visit_merge_grid_merge_client(&mut self, payload: &MyCubeGrid_MergeGrid_MergeClientPayload) {
        println!(
            "Grid '{}' merging with grid: {:?}",
            self.grid.name, payload
        );
        // Mutate the grid instance
    }

    fn visit_on_grid_changed_rpc(&mut self) {
        println!("Grid '{}' changed", self.grid.name);
        // Mark grid as dirty, schedule refresh, etc.
    }
}

/// Using visitors with specific instances.
#[allow(dead_code)]
fn handle_grid_event_with_visitor(
    raw_rpc: &RawRpcPacket,
    grid: &mut GridInstance,
) -> Result<(), Box<dyn std::error::Error>> {
    let schema = Version::embedded();
    let type_hash = ReplicatedType::MyCubeGrid.type_hash();
    let event_hash = raw_rpc.resolve_instance_event_hash(&schema, type_hash)?;

    if let Some(event) = MyCubeGridInstanceEvent::from_hash(event_hash) {
        let payload = event.parse_payload(&raw_rpc.payload)?;
        
        // Create a visitor that has access to THIS specific grid
        let mut handler = GridEventHandler { grid };
        payload.accept(&mut handler);
    }

    Ok(())
}

// =============================================================================
// Multi-version support with dynamic schema loading
// =============================================================================

/// Handle packets from different game versions by loading appropriate schemas.
///
/// Use this when your application needs to work with multiple game versions
/// (e.g., a server browser, replay analyzer, or protocol proxy).
#[allow(dead_code)]
fn handle_multi_version(
    rpc_data: &[u8],
    game_version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load the appropriate schema for this game version
    let schema = if game_version == Version::embedded_version() {
        // Use embedded schema for the version this crate was built against
        Version::embedded()
    } else {
        // Load from file for other versions
        let path = format!("schemas/schema_v{}.json", game_version);
        Version::load(&path)?
    };

    // Parse raw packet (works with any version)
    let raw_rpc = RawRpcPacket::from_bytes(rpc_data)?;

    if raw_rpc.is_static_event() {
        // Parse using version-specific schema
        let payload = raw_rpc.parse_static_event(&schema)?;
        println!("Parsed event from v{}: {:?}", game_version, payload);
    }

    Ok(())
}

// =============================================================================
// Convenience methods
// =============================================================================

/// Quick parsing using the embedded schema (most common use case).
#[allow(dead_code)]
fn quick_parse_static_event(raw_rpc: &RawRpcPacket) -> Result<StaticEventPayload, RpcError> {
    // Shorthand for: raw_rpc.parse_static_event(&Version::embedded())
    raw_rpc.parse_static_event_embedded()
}

// =============================================================================
// Visitor Pattern - handle events without giant match statements
// =============================================================================

/// Example: A visitor that logs chat messages and player creations.
///
/// With the visitor pattern, you only implement handlers for events you care
/// about - all other events have default no-op implementations.
struct ChatAndPlayerLogger {
    chat_count: usize,
    player_count: usize,
}

impl StaticEventVisitor for ChatAndPlayerLogger {
    fn visit_on_chat_message_received_server(&mut self, payload: &OnChatMessageReceived_ServerPayload) {
        self.chat_count += 1;
        println!("[Chat #{}] {:?}", self.chat_count, payload.msg);
    }

    fn visit_on_player_created(&mut self, payload: &OnPlayerCreatedPayload) {
        self.player_count += 1;
        println!(
            "[Player #{}] Steam ID {} joined",
            self.player_count, payload.client_steam_id.0
        );
    }

    // All other ~500 event types automatically have empty default implementations
}

/// Using the visitor pattern for static events.
#[allow(dead_code)]
fn handle_with_visitor(raw_rpc: &RawRpcPacket) -> Result<(), RpcError> {
    let payload = raw_rpc.parse_static_event_embedded()?;

    // Create visitor and dispatch
    let mut visitor = ChatAndPlayerLogger {
        chat_count: 0,
        player_count: 0,
    };
    payload.accept(&mut visitor);

    Ok(())
}

// -----------------------------------------------------------------------------
// Stateful visitor example - collecting data across multiple events
// -----------------------------------------------------------------------------

/// A visitor that collects statistics about events seen.
struct EventCounter {
    total: usize,
    chat_messages: usize,
    faction_events: usize,
    mod_messages: usize,
}

impl EventCounter {
    fn new() -> Self {
        Self {
            total: 0,
            chat_messages: 0,
            faction_events: 0,
            mod_messages: 0,
        }
    }
}

impl StaticEventVisitor for EventCounter {
    fn visit_on_chat_message_received_server(&mut self, _: &OnChatMessageReceived_ServerPayload) {
        self.total += 1;
        self.chat_messages += 1;
    }

    // You can handle entire families of events with similar patterns
    fn visit_mod_message_server_reliable(
        &mut self,
        _: &space_engineers_transport::protocol::ModMessageServerReliablePayload,
    ) {
        self.total += 1;
        self.mod_messages += 1;
    }

    fn visit_mod_message_server_unreliable(
        &mut self,
        _: &space_engineers_transport::protocol::ModMessageServerUnreliablePayload,
    ) {
        self.total += 1;
        self.mod_messages += 1;
    }

    fn visit_unknown(&mut self, _event_hash: i32, _payload: &[u8]) {
        self.total += 1;
    }
}

/// Process multiple events and collect statistics.
#[allow(dead_code)]
fn process_static_event_stream(packets: Vec<&RawRpcPacket>) -> EventCounter {
    let mut counter = EventCounter::new();

    for packet in packets {
        if packet.is_static_event() {
            if let Ok(payload) = packet.parse_static_event_embedded() {
                payload.accept(&mut counter);
            }
        }
    }

    println!(
        "Processed {} events: {} chat, {} mod messages, {} faction",
        counter.total, counter.chat_messages, counter.mod_messages, counter.faction_events
    );

    counter
}

// =============================================================================
// Full packet processing loop example
// =============================================================================

/// Example of a complete event processing loop with state tracking.
#[allow(dead_code)]
fn process_packet_stream(
    packets: Vec<&RawRpcPacket>,
    state: &mut ReplicationState,
) {
    let mut static_visitor = EventCounter::new();
    
    for packet in packets {
        if packet.is_static_event() {
            // Static events (network_id == 0) don't need instance tracking
            if let Ok(payload) = packet.parse_static_event_embedded() {
                payload.accept(&mut static_visitor);
            }
        } else {
            // Instance events need to be routed to the correct object
            if let Err(e) = handle_instance_event_with_state(packet, state) {
                eprintln!("Failed to handle instance event: {}", e);
            }
        }
    }
    
    println!(
        "Static events: {} chat, {} mod messages",
        static_visitor.chat_messages, static_visitor.mod_messages
    );
    println!("Tracked {} grids", state.grids.len());
}

