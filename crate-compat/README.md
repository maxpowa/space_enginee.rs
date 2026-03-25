# space_engineers_compat

Hand-written Rust equivalents for C# types used in Space Engineers.

## Features

- **BCL types**: `DateTime`, `TimeSpan`, `Guid`, `Decimal`
- **Nullable**: `Nullable<T>` wrapper for C# nullable value types
- **Collections**: `VarMap`, `Tuple` types
- **Bit-stream**: `Varint`, `BitAligned`, `VarBytes`, `VarString`, `VarVec` for deku serialization
- **Math**: `Vector3F`, `Vector3D`, `Quaternion`, `Matrix` types
- **Direction**: `Base6Directions::Direction` enum

## Serialization Support

All types support multiple serialization formats:

| Format | Crate | Use case |
|--------|-------|----------|
| XML | `quick-xml` + `serde` | World saves, blueprints |
| JSON | `serde_json` | Diagnostics, debugging |
| Protobuf | `proto_rs` | Server browser data |
| Binary | `deku` | Network packets |

## Usage

This crate is an internal dependency of `space_engineers`. Most users should depend on `space_engineers` directly.

```toml
[dependencies]
space_engineers = "1.208015"
```
