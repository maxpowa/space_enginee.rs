# space_engineers_transport

Network transport layer for Space Engineers — packet framing, replication, and RPC handling.

## Modules

| Module | Purpose |
|--------|---------|
| `packet` | Low-level packet framing with checksums and fragmentation |
| `control` | Server control packets (kick, ban, disconnect, password) |
| `replication` | Replication packet types for client-server sync |
| `rpc` | RPC (Remote Procedure Call) event handling |
| `protocol` | Stable type/event identities and version info (FNV-1a hash-based) |

## Quick Start

```rust
use space_engineers_transport::{StaticRpcPacket, StaticEventPayload};

// Parse an RPC packet using the embedded schema
let (_, packet) = StaticRpcPacket::from_bytes((data, 0))?;

match &packet.payload {
    StaticEventPayload::OnChatMessageReceived_Server(msg) => { /* ... */ },
    StaticEventPayload::OnPlayerCreated(player) => { /* ... */ },
    _ => { /* ... */ },
}
```

## Multi-Version Support

Event IDs change between game versions. For multi-version support:

```rust
use space_engineers_transport::{RawRpcPacket, Version};

// Load schema for a specific game version
let schema = Version::load("schema_v1205026.json")?;
let (_, packet) = RawRpcPacket::from_bytes((data, 0))?;

let payload = packet.parse_static_event(&schema)?;
```

## Examples

```bash
cargo run --example parse_rpc
```
