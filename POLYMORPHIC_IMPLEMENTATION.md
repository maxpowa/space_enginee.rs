# Polymorphic (dynamic) object builders

Some object-builder fields are declared with `[Serialize(MyObjectFlags.Dynamic)]`. Their
static type is an abstract base (`MyObjectBuilder_ToolbarItem`, `MyObjectBuilder_PhysicalObject`,
...), but the wire format starts with the concrete type's runtime id:

```
[uint16 little-endian: MyRuntimeObjectBuilderId][concrete protobuf/deku payload]
```

`MyObjectBuilderSerializerKeen.SerializeDynamic` writes it with `WriteUInt16`, and the ids come
from `MyObjectBuilderType`, which assigns them sequentially (`++m_idCounter`) while registering
assemblies in fully-qualified-name order. **The ids therefore change between game versions and
must be read from the running game — never hardcoded.**

## How it works now

The extractor runs inside the game, so `codegen` can resolve the real ids.

1. **Discovery** (`RustStructGenerator.DiscoverPolymorphicBases`)
   - Walks every reachable network type (and dynamic event parameters) for members with
     `[Serialize(MyObjectFlags.Dynamic)]`.
   - For each declared type, enumerates concrete `MyObjectBuilder_Base` subclasses in the loaded
     assemblies and reads `((MyRuntimeObjectBuilderId)(MyObjectBuilderType)type).Value`.
   - A declared type that is itself concrete is included as its own variant: the wire always
     carries an id, even when the type has no subclasses (e.g. `MyObjectBuilder_Toolbar`).
   - For collections the discriminator sits on each element, so the element type is used
     (`VarVec<ElementVariant>` / `Vec<ElementVariant>`).
   - Pulls those concrete types (and their dependencies) into the Deku type set, so they are
     generated with `DekuRead`/`DekuWrite`.

2. **Enum generation** (`WritePolymorphicEnum`)
   - Emits `<BaseType>Variant` next to the base type (object-builders module):
     ```rust
     #[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, DekuRead, DekuWrite)]
     #[deku(id_type = "u16", id_endian = "little")]
     pub enum MyObjectBuilder_ToolbarItemVariant {
         #[default]
         #[deku(id = "0")]
         Invalid,
         #[deku(id = "244")]
         Animation(Box<MyObjectBuilder_ToolbarItemAnimation>),
         ...
     }
     ```
   - `id = 0` is the null/invalid discriminator (`MyObjectBuilderType.Invalid`).
   - Payloads are boxed to break recursive cycles
     (e.g. `EntityBaseVariant -> Character -> ... -> EntityBaseVariant`).
   - Each enum gets `type_id()`, mirroring the game's id lookup.

3. **Field substitution**
   - Members marked `Dynamic` use the enum type instead of the empty stub.
   - `DefaultZero` still wraps the field in `compat::Nullable<T>` (the has-value bit).
   - Event parameters in `crate-transport` (`ReplicationEvents.GetMethodArgs`) go through the same
     path, so payload structs reference the enum too.

## XML fallback (`xml-fragment`)

The opaque tree lives in its own crate and repository, [`xml-fragment`](https://github.com/maxpowa/xml-fragment.rs), published on crates.io, and is re-exported
as `space_engineers_compat::XmlFragment` (so generated code keeps using
`crate::compat::XmlFragment`). Neither quick-xml nor the serde XML crates provide an opaque
subtree type, so it is maintained here.

The enums implement serde manually instead of deriving it:

- **Deserialize** captures the whole element into an `XmlFragment` (maps, sequences, scalars;
  attributes are `@name` keys). It then reads `xsi:type` and tries to parse the fragment into the
  matching concrete type by rendering it back to XML and running quick-xml over it. Unknown types,
  or payloads whose generated field names do not match (e.g. the existing `defId` vs
  `DefinitionId` mismatch), become `Unknown(Box<XmlFragment>)` instead of failing.
- **Serialize** writes typed variants as `<Field xsi:type="Concrete">...` and `Unknown` back from
  the captured fragment unchanged.

So XML never loses data: unknown or unparseable payloads round-trip structurally. The fragment
preserves element names, attributes, nesting and repeated elements; it does not preserve
whitespace, attribute order, namespace prefixes, or `<X/>` vs `<X></X>`.

The enums still derive `DekuRead`/`DekuWrite` with the real runtime ids. `Unknown` uses a sentinel
id (`65535`, far above the registry's current ~1000 entries) and wraps
`compat::XmlOnlyFragment`, a local newtype that delegates serde to the fragment and implements the
binary traits as a placeholder (read yields nothing, write fails). Keeping that wrapper in
`compat` means the published `xml-fragment` crate does not depend on deku; deku can never produce
the sentinel from binary data.

## Deliberate limits

- **Bounded hierarchies only.** `MaxPolymorphicVariants = 150`. Unbounded bases
  (`MyObjectBuilder_Base` alone has 972 subtypes, though its only dynamic field is the
  debug-only `MyInventory.DebugAddItems` RPC) would drag most of the game's object-builders into
  the crate and break unrelated generated code (protobuf derives, name collisions). Their dynamic
  fields keep the previous stub type. Raise the cap once the supporting issues are fixed.
- **Protobuf is skipped for polymorphic-only types.** Concrete variants that only became reachable
  through a dynamic field are emitted without `#[proto_rs::proto_message]`, and the enum field gets
  `#[proto(skip)]`; their shapes do not always satisfy proto_rs' bounds.
- **Duplicate Rust names are skipped with a warning.** Nested enums whose generated name collides
  (e.g. `Opening_MoveType` from two different outer types) keep the first definition.

## Regenerating

```powershell
.\codegen\generate.ps1          # builds the extractor and runs it against the installed game
cargo check --workspace
cargo test --workspace
```

The extractor prints any dynamic member it had to leave as a stub (over-cap bases) so they can be
triaged.
