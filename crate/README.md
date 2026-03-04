# Space Engineers 1 Data Crate

This crate contains data structures and serialization logic for Space Engineers 1 (SE1) game data. It includes
various modules for handling game objects, networking, and serialization formats used in SE1.

## Code Generation

Some of the Rust source files in `src/` are **generated** by the C# codegen tool located in `../codegen/`.
The codegen tool launches Space Engineers, reflects over its assemblies, and outputs `.rs` files containing
struct/enum definitions with serde + protobuf derive macros.

### Generated files

| Crate module       | Source type(s)                        |
|--------------------|---------------------------------------|
| `server_data.rs`   | `MyCachedServerItem.MyServerData`     |
| `packets.rs`       | `MyObjectBuilder_World`               |
| `rpc_types.rs`     | All replication event parameter types |

JSON diagnostic files (`id_to_type.json`, `static_events.json`) are written to
`<Bin64>/SEProtoExtractor/<version>/`.

### Quick start

From the repository root, run the PowerShell script:

```powershell
.\generate.ps1
```

The script builds the C# extractor, runs it against Space Engineers, and writes the `.rs`
files directly into `crate/src/`.

If SE is not installed via Steam (or the registry key is missing), pass the path explicitly:

```powershell
.\generate.ps1 -Bin64 "C:\Program Files (x86)\Steam\steamapps\common\SpaceEngineers\Bin64"
```

### Manual workflow

1. Build the codegen project: `dotnet build codegen/ -c Debug`
2. Run the extractor with `--output` pointing at the crate:
   ```
   codegen\bin\Debug\netframework48\StandaloneExtractor.exe --output crate\src
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

