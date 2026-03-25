# space_engineers

Public API crate for Space Engineers 1 (SE1) data structures and serialization.

## Usage

```toml
[dependencies]
space_engineers = "1.208015"
```

This crate re-exports types from the internal `space_engineers_compat` and `space_engineers_sys` crates.

## Features

- **World saves**: Parse and serialize `MyObjectBuilder_World` (blueprints, saves)
- **Server data**: `MyServerData` for server browser integration
- **Math types**: Vectors, matrices, quaternions matching SE's coordinate system
- **BCL types**: `DateTime`, `Guid`, `TimeSpan` with C# compatibility

## Related Crates

| Crate | Purpose |
|-------|---------|
| `space_engineers_compat` | Hand-written compatibility types |
| `space_engineers_sys` | Auto-generated SE struct definitions |
| `space_engineers_transport` | Network packet parsing |

## Code Generation

Some types in this workspace are auto-generated. See the [repository README](../README.md) for codegen instructions.

