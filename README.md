# space_enginee.rs

Rust data structures and serialization for Space Engineers 1 (SE1).

## Crate structure

| Crate | Path | Purpose |
|---|---|---|
| `space_engineers_compat` | `crate-compat/` | Hand-written compatibility types (DateTime, Guid, BitField, math vectors, etc.) |
| `space_engineers_sys` | `crate-sys/` | Auto-generated SE struct/enum definitions (codegen output) |
| `space_engineers_transport` | `crate-transport/` | Network transport layer — packet framing, replication, RPC, and type identities |
| `space_engineers` | `crate/` | Public API — re-exports compat + sys, adds macros and higher-level features |

Consumers depend on `space_engineers` only. The compat and sys crates are implementation details
that can be versioned and published independently.

## Code Generation

The generated Rust files are produced by the C# codegen tool in `codegen/`.
The tool launches Space Engineers, reflects over its assemblies, and outputs `.rs` files with
serde + deku derive macros.

### Generated files

| Path | Contents |
|---|---|
| `crate-sys/src/types.rs` | `MyServerData`, `MyObjectBuilder_World`, and all replication event parameter types |
| `crate-transport/src/protocol/` | Stable type/event identities (FNV-1a hash-based enums) |
| `crate-transport/src/protocol/version/embedded_schema.json` | Version-specific ID-to-hash mappings for runtime resolution |

JSON diagnostic files (`id_to_type.json`, `static_events.json`, `schema_v*.json`) are written to
`<Bin64>/SEProtoExtractor/<version>/`.

### Quick start

From the repository root, run the PowerShell script:

```powershell
.\codegen\generate.ps1
```

The script builds the C# extractor, runs it against Space Engineers, and writes the generated
files into `crate-sys/src/` and `crate-transport/src/`.

If SE is not installed via Steam (or the registry key is missing), pass the path explicitly:

```powershell
.\codegen\generate.ps1 -Bin64 "C:\Program Files (x86)\Steam\steamapps\common\SpaceEngineers\Bin64"
```

### Manual workflow

1. Build the codegen project: `dotnet build codegen/StandaloneExtractor.csproj -c Debug`
2. Run the extractor with `--output` pointing at the sys crate (transport path is inferred as sibling):
   ```
   codegen\bin\Debug\netframework48\StandaloneExtractor.exe --output crate-sys\src
   ```
   If not running from the SE Bin64 directory, also pass `--bin64 <path>`.

## Notes

- As of Dec 3 2025 SE1 only requires `MyCachedServerItem.MyServerData` to be protobuf, nothing else uses protobuf
  except mods and the texture cache.
    - Ref: Sandbox.ModAPI.MyAPIUtilities.SerializeToBinary[T]
    - Ref: Sandbox.Game.MyCachedServerItem.GetServerData
    - Ref: VRage.Render11.Resources.MyTextureCache.Save
- We can probably strip out a lot of the Protobuf definitions that are not needed for basic functionality if needed to
  better support the deku structs
- Mods may use protobuf for any object, so we can't really completely strip them out