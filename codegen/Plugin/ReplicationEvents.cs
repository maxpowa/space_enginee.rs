using System.Diagnostics.CodeAnalysis;
using System.Reflection;
using System.Text;
using System.Text.RegularExpressions;
using HarmonyLib;
using Newtonsoft.Json;
using Sandbox.Engine.Multiplayer;
using VRage.Network;

namespace StandaloneExtractor.Plugin;

public class ReplicationEvents
{
    private readonly string _outputPath;
    private readonly MyTypeTable _typeTable;
    private readonly List<MySynchronizedTypeInfo> _typeIndexedList;

    public ReplicationEvents(string outputPath)
    {
        this._outputPath = outputPath;

        var replicationLayer = new MyReplicationSingle(new EndpointId(0));
        replicationLayer.RegisterFromGameAssemblies();

        this._typeTable = Traverse.Create(replicationLayer).Field("m_typeTable").GetValue() as MyTypeTable ??
                          throw new InvalidOperationException("Couldn't load type table");
        this._typeIndexedList =
            Traverse.Create(_typeTable).Field("m_idToType").GetValue() as List<MySynchronizedTypeInfo> ??
            throw new InvalidOperationException("Couldn't load type indexed list");
    }

    private static Dictionary<uint, CallSite> GetEventTableMapping(MyEventTable eventTable)
    {
        return Traverse.Create(eventTable).Field("m_idToEvent").GetValue() as Dictionary<uint, CallSite> ??
               throw new InvalidOperationException("Couldn't get event table mapping");
    }

    public List<Type> GetAllTypes()
    {
        var list = new List<Type>();
        list.AddList(
            _typeIndexedList
                .SelectMany(each =>
                GetEventTableMapping(each.EventTable)
                        .SelectMany(pair => pair.Value.MethodInfo.GetParameters().Types())).ToList()
        );
        list.AddList(
            GetEventTableMapping(_typeTable.StaticEventTable)
                .SelectMany(pair => pair.Value.MethodInfo.GetParameters().Types()).ToList()
        );
        return list.Distinct().ToList();
    }
    
    [SuppressMessage("ReSharper", "NotAccessedField.Local")]
    private class EventTypeInfo
    {
        public uint Id;
        public bool IsProxied;
        public int ParentTypeHash;
        public bool IsReliable;
        public bool IsBlocking;
        public string CallSiteFlags;
        public string ValidationFlags;
        public string MethodDescriptor;
    }
    
    private static EventTypeInfo BuildEventTypeInfo(KeyValuePair<uint, CallSite> pair)
    {
        // REF: VRage.Network.MyReplicationLayer.DispatchEvent[T1, T2, T3, T4, T5, T6, T7, T8]
        // If the first parameter is IMyEventProxy, then it's a proxy event.
        var arg1 = pair.Value.MethodInfo.GetParameters().FirstOrDefault();
        var isProxied = arg1 != null && typeof(IMyEventProxy).IsAssignableFrom(arg1.ParameterType);
        
        return new EventTypeInfo{
            Id = pair.Value.Id,
            IsProxied = isProxied,
            ParentTypeHash = MySynchronizedTypeInfo.GetHashFromType(pair.Value.MethodInfo.DeclaringType),
            IsReliable = pair.Value.IsReliable,
            IsBlocking = pair.Value.IsBlocking,
            CallSiteFlags = pair.Value.CallSiteFlags.ToString(),
            ValidationFlags = pair.Value.ValidationFlags.ToString(),
            MethodDescriptor = pair.Value.MethodInfo.ToString()!,
        };
    }

    /// <summary>
    /// Serializes the current loaded assemblies RPC objects and their internal events, as well as the global events list.
    /// </summary>
    public void Serialize()
    {
        var eventListFilename = Path.Combine(_outputPath, "id_to_type.json");
        using (var writer = new StreamWriter(eventListFilename))
        using (var json = new JsonTextWriter(writer))
        {
            json.Formatting = Formatting.Indented;
            var ser = new JsonSerializer();
            ser.Serialize(json, _typeIndexedList.Select((x, i) =>
            {
                var events = GetEventTableMapping(x.EventTable);
                var count = this._typeTable.Get(x.Type).EventTable.Count;
                var isNetProxy = typeof(IMyEventProxy).IsAssignableFrom(x.Type);
                return new
                {
                    Id = i, // This index is the same as the MySynchronizedTypeInfo.TypeId, so just use this for simplicity
                    x.TypeHash,
                    x.IsReplicated,
                    x.FullTypeName,
                    InstanceEvents = events.Select(BuildEventTypeInfo)
                };
            }));
        }

        var eventTable = _typeTable.StaticEventTable;
        var eventTableIdMapping = eventTable != null
            ? GetEventTableMapping(eventTable)
            : throw new Exception("Couldn't get static event table mapping");
        var eventTypesFilename = Path.Combine(_outputPath, "static_events.json");
        using (var writer = new StreamWriter(eventTypesFilename))
        using (var json = new JsonTextWriter(writer))
        {
            json.Formatting = Formatting.Indented;
            var ser = new JsonSerializer();
            ser.Serialize(json, eventTableIdMapping.Select(BuildEventTypeInfo));
        }
    }
    
    private static string GetShortTypeName(string fullTypeName)
    {
        // Extract just the class name from "Namespace.ClassName" or "Namespace.Outer+Inner"
        var lastDot = fullTypeName.LastIndexOf('.');
        var name = lastDot >= 0 ? fullTypeName.Substring(lastDot + 1) : fullTypeName;
        // Replace + with _ for nested types
        name = name.Replace('+', '_');
        return name;
    }

    private static string GetMethodName(MethodInfo method)
    {
        // Get the method name, sanitize for Rust identifier
        var name = method.Name;
        // Remove any generic arity markers
        name = Regex.Replace(name, @"`\d+", "");
        return name;
    }
    
    private static List<(string Name, string RustType)> GetMethodArgs(MethodInfo method)
    {
        var result = new List<(string Name, string RustType)>();
        var parameters = method.GetParameters();
        
        foreach (var param in parameters)
        {
            // Skip IMyEventProxy parameters (these are the target object, not payload data)
            if (typeof(IMyEventProxy).IsAssignableFrom(param.ParameterType))
                continue;
            
            var rustType = RustStructGenerator.DekuTypeNameForTransport(param.ParameterType);
            var name = StringToSnakeCase(param.Name ?? $"arg{result.Count}");
            
            result.Add((name, rustType));
        }
        
        return result;
    }
    
    private static readonly HashSet<string> RustKeywords = new()
    {
        "as", "break", "const", "continue", "crate", "else", "enum", "extern",
        "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
        "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct",
        "super", "trait", "true", "type", "unsafe", "use", "where", "while",
        "async", "await", "dyn", "abstract", "become", "box", "do", "final",
        "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try"
    };
    
    private static string StringToSnakeCase(string input)
    {
        if (string.IsNullOrEmpty(input)) return input;
        
        var sb = new StringBuilder();
        for (int i = 0; i < input.Length; i++)
        {
            var c = input[i];
            if (char.IsUpper(c))
            {
                // Add underscore before uppercase if previous char is not uppercase AND not already an underscore
                if (i > 0 && !char.IsUpper(input[i - 1]) && input[i - 1] != '_')
                    sb.Append('_');
                sb.Append(char.ToLower(c));
            }
            else
            {
                sb.Append(c);
            }
        }
        
        var result = sb.ToString();
        
        // Collapse multiple underscores into one (safety net)
        while (result.Contains("__"))
            result = result.Replace("__", "_");
        
        // Remove leading underscore if any
        result = result.TrimStart('_');
        
        return RustKeywords.Contains(result) ? $"r#{result}" : result;
    }
    
    /// <summary>
    /// Generates a version-specific schema JSON file for runtime type table lookup.
    /// This schema maps version-specific indices to stable type hashes.
    /// </summary>
    /// <param name="outputPath">Directory to write the schema file</param>
    /// <param name="gameVersion">Game version number</param>
    /// <param name="embedded">If true, writes as embedded_schema.json for Rust include_str!</param>
    public void GenerateVersionSchema(string outputPath, int gameVersion, bool embedded = false)
    {
        var versionStr = gameVersion.ToString();
        var schemaFilename = embedded 
            ? Path.Combine(outputPath, "embedded_schema.json")
            : Path.Combine(outputPath, $"schema_v{versionStr}.json");
        
        using var writer = new StreamWriter(schemaFilename);
        using var json = new JsonTextWriter(writer) { Formatting = embedded ? Formatting.None : Formatting.Indented };
        
        var staticEvents = GetEventTableMapping(_typeTable.StaticEventTable);
        
        var schema = new
        {
            game_version = versionStr,
            generated_at = DateTime.UtcNow.ToString("O"),
            
            // Type table: index -> (hash, name)
            types = _typeIndexedList.Select((t, i) => new
            {
                index = (ushort)i,
                hash = t.TypeHash,
                name = t.FullTypeName,
                is_replicated = t.IsReplicated
            }),
            
            // Static events: id -> (hash, name, declaring_type_hash)
            static_events = staticEvents
                .OrderBy(e => e.Key)
                .Select(e => new
                {
                    id = (ushort)e.Key,
                    hash = ComputeEventHash(e.Value.MethodInfo),
                    name = GetMethodName(e.Value.MethodInfo),
                    declaring_type_hash = MySynchronizedTypeInfo.GetHashFromType(e.Value.MethodInfo.DeclaringType!),
                    is_reliable = e.Value.IsReliable
                }),
            
            // Instance events per type: type_hash -> [event mappings]
            instance_events = _typeIndexedList
                .Where(t => GetEventTableMapping(t.EventTable).Any())
                .Select(t => new
                {
                    type_hash = t.TypeHash,
                    type_name = t.FullTypeName,
                    events = GetEventTableMapping(t.EventTable)
                        .OrderBy(e => e.Key)
                        .Select(e => new
                        {
                            id = (ushort)e.Key,
                            hash = ComputeEventHash(e.Value.MethodInfo),
                            name = GetMethodName(e.Value.MethodInfo),
                            is_reliable = e.Value.IsReliable
                        })
                })
        };
        
        new JsonSerializer().Serialize(json, schema);
        Console.WriteLine($"  Schema JSON: {schemaFilename}");
    }
    
    /// <summary>
    /// Generates stable Rust identity types that don't change between versions.
    /// These use type hashes as the stable identifier, not version-specific indices.
    /// Also generates payload structs and a dispatch enum for parsing event payloads.
    /// Splits output into multiple files for easier navigation.
    /// </summary>
    public void GenerateRustIdentityTypes(string rsOutputPath)
    {
        var identityDir = Path.Combine(rsOutputPath, "protocol");
        Directory.CreateDirectory(identityDir);
        
        // Collect all types that have events (these are the ones we care about for network)
        var typesWithEvents = new List<(int TypeHash, string FullTypeName, string ShortName)>();
        
        for (int i = 0; i < _typeIndexedList.Count; i++)
        {
            var typeInfo = _typeIndexedList[i];
            var events = GetEventTableMapping(typeInfo.EventTable);
            
            if (events.Count > 0 || typeInfo.IsReplicated)
            {
                var shortName = GetShortTypeName(typeInfo.FullTypeName);
                typesWithEvents.Add((typeInfo.TypeHash, typeInfo.FullTypeName, shortName));
            }
        }
        
        // Track name collisions
        var shortNameCounts = typesWithEvents
            .GroupBy(t => t.ShortName)
            .ToDictionary(g => g.Key, g => g.Count());
        
        string GetUniqueTypeName((int TypeHash, string FullTypeName, string ShortName) typeInfo)
        {
            if (shortNameCounts[typeInfo.ShortName] > 1)
            {
                return $"{typeInfo.ShortName}_{typeInfo.TypeHash:X8}";
            }
            return typeInfo.ShortName;
        }
        
        // Generate replicated_types.rs
        GenerateReplicatedTypesFile(identityDir, typesWithEvents, GetUniqueTypeName);
        
        // Generate static_events directory (split by declaring type)
        GenerateStaticEventsFiles(identityDir);
        
        // Generate instance_events directory (split by replicated type)
        GenerateInstanceEventsFile(identityDir, typesWithEvents, GetUniqueTypeName);
        
        Console.WriteLine($"  Identity types: {identityDir}/");
    }
    
    private void GenerateReplicatedTypesFile(
        string identityDir,
        List<(int TypeHash, string FullTypeName, string ShortName)> typesWithEvents,
        Func<(int TypeHash, string FullTypeName, string ShortName), string> GetUniqueTypeName)
    {
        var filename = Path.Combine(identityDir, "replicated_types.rs");
        using var writer = new StreamWriter(filename);
        
        writer.WriteLine("//! Replicated type identities.");
        writer.WriteLine("//!");
        writer.WriteLine("//! Auto-generated by codegen. Do not edit manually.");
        writer.WriteLine();
        
        // Generate ReplicatedType enum (stable, no discriminants)
        writer.WriteLine("/// Stable replicated type identity based on type hash.");
        writer.WriteLine("///");
        writer.WriteLine("/// Use `Version` to convert to/from version-specific indices.");
        writer.WriteLine("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]");
        writer.WriteLine("pub enum ReplicatedType {");
        
        foreach (var typeInfo in typesWithEvents)
        {
            var uniqueName = GetUniqueTypeName(typeInfo);
            writer.WriteLine($"    /// {typeInfo.FullTypeName}");
            writer.WriteLine($"    {uniqueName},");
        }
        
        writer.WriteLine("}");
        writer.WriteLine();
        
        // Generate type_hash() method
        writer.WriteLine("impl ReplicatedType {");
        writer.WriteLine("    /// Get the stable FNV-1a hash of the type name.");
        writer.WriteLine("    pub const fn type_hash(&self) -> i32 {");
        writer.WriteLine("        match self {");
        foreach (var typeInfo in typesWithEvents)
        {
            var uniqueName = GetUniqueTypeName(typeInfo);
            writer.WriteLine($"            Self::{uniqueName} => {typeInfo.TypeHash},");
        }
        writer.WriteLine("        }");
        writer.WriteLine("    }");
        writer.WriteLine();
        
        // Generate from_hash() method
        writer.WriteLine("    /// Look up from type hash. Returns None for unknown types.");
        writer.WriteLine("    pub fn from_hash(hash: i32) -> Option<Self> {");
        writer.WriteLine("        match hash {");
        foreach (var typeInfo in typesWithEvents)
        {
            var uniqueName = GetUniqueTypeName(typeInfo);
            writer.WriteLine($"            {typeInfo.TypeHash} => Some(Self::{uniqueName}),");
        }
        writer.WriteLine("            _ => None,");
        writer.WriteLine("        }");
        writer.WriteLine("    }");
        writer.WriteLine();
        
        // Generate type_name() method
        writer.WriteLine("    /// Get the full type name.");
        writer.WriteLine("    pub const fn type_name(&self) -> &'static str {");
        writer.WriteLine("        match self {");
        foreach (var typeInfo in typesWithEvents)
        {
            var uniqueName = GetUniqueTypeName(typeInfo);
            writer.WriteLine($"            Self::{uniqueName} => \"{typeInfo.FullTypeName}\",");
        }
        writer.WriteLine("        }");
        writer.WriteLine("    }");
        writer.WriteLine("}");
    }
    
    /// <summary>
    /// Gets the category for a static event based on declaring type namespace.
    /// </summary>
    private static string GetStaticEventCategory(Type? declaringType)
    {
        if (declaringType == null) return "other";
        
        var fullName = declaringType.FullName ?? declaringType.Name;
        var parts = fullName.Split('.');
        
        if (parts.Length >= 3)
        {
            var segment = parts[2].ToLowerInvariant();
            return segment switch
            {
                "multiplayer" => "multiplayer",
                "session" => "session",
                "world" => "world",
                "gui" => "gui",
                "entities" => "entities",
                "replication" => "replication",
                _ => "other"
            };
        }
        return "other";
    }
    
    private void GenerateStaticEventsFiles(string identityDir)
    {
        // Create static_events subdirectory
        var staticEventsDir = Path.Combine(identityDir, "static_events");
        Directory.CreateDirectory(staticEventsDir);
        
        var staticEvents = GetEventTableMapping(_typeTable.StaticEventTable)
            .OrderBy(e => e.Key)
            .ToList();
        
        // Build unique names (same logic as before)
        var methodNameCounts = staticEvents
            .GroupBy(e => GetMethodName(e.Value.MethodInfo))
            .ToDictionary(g => g.Key, g => g.Count());
        
        string GetQualifiedEventName(MethodInfo method)
        {
            var baseName = GetMethodName(method);
            if (methodNameCounts[baseName] > 1 && method.DeclaringType != null)
            {
                return $"{method.DeclaringType.Name}_{baseName}";
            }
            return baseName;
        }
        
        var qualifiedNameCounts = staticEvents
            .GroupBy(e => GetQualifiedEventName(e.Value.MethodInfo))
            .ToDictionary(g => g.Key, g => g.Count());
        
        string GetUniqueEventName(uint id, MethodInfo method)
        {
            var qualifiedName = GetQualifiedEventName(method);
            return qualifiedNameCounts[qualifiedName] > 1 ? $"{qualifiedName}_{id}" : qualifiedName;
        }
        
        // Group events by declaring type
        var eventsByDeclaringType = staticEvents
            .GroupBy(e => e.Value.MethodInfo.DeclaringType)
            .Where(g => g.Key != null)
            .ToDictionary(g => g.Key!, g => g.ToList());
        
        // Group declaring types by category
        var categorizedDeclaringTypes = new Dictionary<string, List<Type>>();
        foreach (var declaringType in eventsByDeclaringType.Keys)
        {
            var category = GetStaticEventCategory(declaringType);
            if (!categorizedDeclaringTypes.ContainsKey(category))
                categorizedDeclaringTypes[category] = new List<Type>();
            categorizedDeclaringTypes[category].Add(declaringType);
        }
        
        // Create category directories and generate per-type files
        var allModules = new List<(string Category, string ModuleName, Type DeclaringType)>();
        
        foreach (var (category, declaringTypes) in categorizedDeclaringTypes)
        {
            var categoryDir = Path.Combine(staticEventsDir, category);
            Directory.CreateDirectory(categoryDir);
            
            foreach (var declaringType in declaringTypes.OrderBy(t => t.Name))
            {
                var moduleName = StringToSnakeCase(declaringType.Name);
                allModules.Add((category, moduleName, declaringType));
                
                GenerateStaticEventTypeFile(
                    categoryDir, 
                    moduleName, 
                    declaringType, 
                    eventsByDeclaringType[declaringType],
                    GetUniqueEventName);
            }
            
            // Generate category mod.rs
            GenerateStaticEventCategoryModFile(categoryDir, declaringTypes);
        }
        
        // Generate root static_events/mod.rs with unified types
        GenerateStaticEventsRootModFile(
            staticEventsDir, 
            categorizedDeclaringTypes, 
            staticEvents, 
            eventsByDeclaringType,
            GetUniqueEventName);
    }
    
    private void GenerateStaticEventTypeFile(
        string categoryDir,
        string moduleName,
        Type declaringType,
        List<KeyValuePair<uint, CallSite>> events,
        Func<uint, MethodInfo, string> GetUniqueEventName)
    {
        var filename = Path.Combine(categoryDir, $"{moduleName}.rs");
        using var writer = new StreamWriter(filename);
        
        writer.WriteLine($"//! Static event payloads declared in {declaringType.FullName}.");
        writer.WriteLine("//!");
        writer.WriteLine("//! Auto-generated by codegen. Do not edit manually.");
        writer.WriteLine();
        writer.WriteLine("#![allow(unused_imports)]");
        writer.WriteLine();
        writer.WriteLine("use deku::prelude::*;");
        writer.WriteLine("use space_engineers_compat::{BitAligned, BitBool, Nullable, VarMap, VarVec, VarBytes, VarString};");
        writer.WriteLine("use space_engineers_compat::math::Vector3F;");
        writer.WriteLine("use space_engineers_sys::math::Vector3D;");
        writer.WriteLine();
        
        // Generate payload structs for events with parameters
        foreach (var ev in events.OrderBy(e => e.Key))
        {
            var eventName = GetUniqueEventName(ev.Key, ev.Value.MethodInfo);
            var args = GetMethodArgs(ev.Value.MethodInfo);
            
            if (args.Count > 0)
            {
                writer.WriteLine($"/// Payload for {eventName} event.");
                writer.WriteLine("#[derive(Debug, Clone, PartialEq, DekuRead, DekuWrite)]");
                writer.WriteLine($"pub struct {eventName}Payload {{");
                foreach (var (name, rustType) in args)
                {
                    writer.WriteLine($"    pub {name}: {rustType},");
                }
                writer.WriteLine("}");
                writer.WriteLine();
            }
        }
    }
    
    private void GenerateStaticEventCategoryModFile(string categoryDir, List<Type> declaringTypes)
    {
        var filename = Path.Combine(categoryDir, "mod.rs");
        using var writer = new StreamWriter(filename);
        
        writer.WriteLine("//! Static event payloads by declaring type.");
        writer.WriteLine("//!");
        writer.WriteLine("//! Auto-generated by codegen. Do not edit manually.");
        writer.WriteLine();
        
        foreach (var declaringType in declaringTypes.OrderBy(t => t.Name))
        {
            var moduleName = StringToSnakeCase(declaringType.Name);
            writer.WriteLine($"mod {moduleName};");
        }
        
        writer.WriteLine();
        
        foreach (var declaringType in declaringTypes.OrderBy(t => t.Name))
        {
            var moduleName = StringToSnakeCase(declaringType.Name);
            writer.WriteLine($"pub use {moduleName}::*;");
        }
    }
    
    private void GenerateStaticEventsRootModFile(
        string staticEventsDir,
        Dictionary<string, List<Type>> categorizedDeclaringTypes,
        List<KeyValuePair<uint, CallSite>> staticEvents,
        Dictionary<Type, List<KeyValuePair<uint, CallSite>>> eventsByDeclaringType,
        Func<uint, MethodInfo, string> GetUniqueEventName)
    {
        var filename = Path.Combine(staticEventsDir, "mod.rs");
        using var writer = new StreamWriter(filename);
        
        writer.WriteLine("//! Static event identities and payloads.");
        writer.WriteLine("//!");
        writer.WriteLine("//! Auto-generated by codegen. Do not edit manually.");
        writer.WriteLine();
        writer.WriteLine("#![allow(unused_imports)]");
        writer.WriteLine();
        
        // Module declarations
        foreach (var category in categorizedDeclaringTypes.Keys.OrderBy(c => c))
        {
            writer.WriteLine($"mod {category};");
        }
        writer.WriteLine();
        
        // Re-exports
        foreach (var category in categorizedDeclaringTypes.Keys.OrderBy(c => c))
        {
            writer.WriteLine($"pub use {category}::*;");
        }
        writer.WriteLine();
        
        writer.WriteLine("use deku::prelude::*;");
        writer.WriteLine();
        
        // Collect events with payloads
        var eventsWithPayloads = new HashSet<string>();
        foreach (var ev in staticEvents)
        {
            var args = GetMethodArgs(ev.Value.MethodInfo);
            if (args.Count > 0)
            {
                var eventName = GetUniqueEventName(ev.Key, ev.Value.MethodInfo);
                eventsWithPayloads.Add(eventName);
            }
        }
        
        // Generate StaticEventType enum
        writer.WriteLine("/// Stable static event identity based on event hash.");
        writer.WriteLine("///");
        writer.WriteLine("/// Use `Version` to convert to/from version-specific event IDs.");
        writer.WriteLine("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]");
        writer.WriteLine("pub enum StaticEventType {");
        
        foreach (var ev in staticEvents)
        {
            var eventName = GetUniqueEventName(ev.Key, ev.Value.MethodInfo);
            var eventHash = ComputeEventHash(ev.Value.MethodInfo);
            writer.WriteLine($"    /// Hash: {eventHash}");
            writer.WriteLine($"    {eventName},");
        }
        
        writer.WriteLine("}");
        writer.WriteLine();
        
        // Generate event_hash() and from_hash()
        writer.WriteLine("impl StaticEventType {");
        writer.WriteLine("    /// Get the stable hash of this event.");
        writer.WriteLine("    pub const fn event_hash(&self) -> i32 {");
        writer.WriteLine("        match self {");
        foreach (var ev in staticEvents)
        {
            var eventName = GetUniqueEventName(ev.Key, ev.Value.MethodInfo);
            var eventHash = ComputeEventHash(ev.Value.MethodInfo);
            writer.WriteLine($"            Self::{eventName} => {eventHash},");
        }
        writer.WriteLine("        }");
        writer.WriteLine("    }");
        writer.WriteLine();
        
        writer.WriteLine("    /// Look up from event hash. Returns None for unknown events.");
        writer.WriteLine("    pub fn from_hash(hash: i32) -> Option<Self> {");
        writer.WriteLine("        match hash {");
        foreach (var ev in staticEvents)
        {
            var eventName = GetUniqueEventName(ev.Key, ev.Value.MethodInfo);
            var eventHash = ComputeEventHash(ev.Value.MethodInfo);
            writer.WriteLine($"            {eventHash} => Some(Self::{eventName}),");
        }
        writer.WriteLine("            _ => None,");
        writer.WriteLine("        }");
        writer.WriteLine("    }");
        writer.WriteLine("}");
        writer.WriteLine();
        
        // Generate StaticEventPayload enum
        writer.WriteLine("/// Parsed static event payload, dispatched by event identity.");
        writer.WriteLine("///");
        writer.WriteLine("/// Use `StaticEventType::parse_payload()` to parse raw bytes into this enum.");
        writer.WriteLine("#[derive(Debug, Clone, PartialEq)]");
        writer.WriteLine("pub enum StaticEventPayload {");
        
        foreach (var ev in staticEvents)
        {
            var eventName = GetUniqueEventName(ev.Key, ev.Value.MethodInfo);
            var hasPayload = eventsWithPayloads.Contains(eventName);
            
            if (hasPayload)
            {
                writer.WriteLine($"    /// {eventName} event payload");
                writer.WriteLine($"    {eventName}({eventName}Payload),");
            }
            else
            {
                writer.WriteLine($"    /// {eventName} event (no payload)");
                writer.WriteLine($"    {eventName},");
            }
        }
        
        writer.WriteLine("    /// Unknown event (raw bytes preserved)");
        writer.WriteLine("    Unknown { event_hash: i32, payload: Vec<u8> },");
        writer.WriteLine("}");
        writer.WriteLine();
        
        // Add parse_payload method to StaticEventType
        writer.WriteLine("#[allow(unused_variables)]");
        writer.WriteLine("impl StaticEventType {");
        writer.WriteLine("    /// Parse the payload bytes for this event type.");
        writer.WriteLine("    ///");
        writer.WriteLine("    /// # Errors");
        writer.WriteLine("    ///");
        writer.WriteLine("    /// Returns `Err` if the payload bytes don't match the expected format.");
        writer.WriteLine("    pub fn parse_payload(&self, bytes: &[u8]) -> Result<StaticEventPayload, deku::DekuError> {");
        writer.WriteLine("        match self {");
        
        foreach (var ev in staticEvents)
        {
            var eventName = GetUniqueEventName(ev.Key, ev.Value.MethodInfo);
            var hasPayload = eventsWithPayloads.Contains(eventName);
            
            if (hasPayload)
            {
                writer.WriteLine($"            Self::{eventName} => {{");
                writer.WriteLine($"                let (_, payload) = {eventName}Payload::from_bytes((bytes, 0))?;");
                writer.WriteLine($"                Ok(StaticEventPayload::{eventName}(payload))");
                writer.WriteLine("            }");
            }
            else
            {
                writer.WriteLine($"            Self::{eventName} => Ok(StaticEventPayload::{eventName}),");
            }
        }
        
        writer.WriteLine("        }");
        writer.WriteLine("    }");
        writer.WriteLine("}");
        writer.WriteLine();
        
        // Helper to parse from raw event hash + bytes
        writer.WriteLine("/// Parse a static event payload from raw event hash and bytes.");
        writer.WriteLine("///");
        writer.WriteLine("/// This is a convenience function that combines hash lookup with payload parsing.");
        writer.WriteLine("pub fn parse_static_event(event_hash: i32, payload_bytes: &[u8]) -> Result<StaticEventPayload, deku::DekuError> {");
        writer.WriteLine("    match StaticEventType::from_hash(event_hash) {");
        writer.WriteLine("        Some(event_type) => event_type.parse_payload(payload_bytes),");
        writer.WriteLine("        None => Ok(StaticEventPayload::Unknown {");
        writer.WriteLine("            event_hash,");
        writer.WriteLine("            payload: payload_bytes.to_vec(),");
        writer.WriteLine("        }),");
        writer.WriteLine("    }");
        writer.WriteLine("}");
        writer.WriteLine();
        
        // Generate StaticEventVisitor trait
        writer.WriteLine("// =============================================================================");
        writer.WriteLine("// Visitor Pattern");
        writer.WriteLine("// =============================================================================");
        writer.WriteLine();
        writer.WriteLine("/// Visitor trait for static events.");
        writer.WriteLine("///");
        writer.WriteLine("/// Implement only the methods you care about - all have default empty implementations.");
        writer.WriteLine("#[allow(unused_variables)]");
        writer.WriteLine("pub trait StaticEventVisitor {");
        
        foreach (var ev in staticEvents)
        {
            var eventName = GetUniqueEventName(ev.Key, ev.Value.MethodInfo);
            var snakeName = StringToSnakeCase(eventName);
            var hasPayload = eventsWithPayloads.Contains(eventName);
            
            if (hasPayload)
            {
                writer.WriteLine($"    /// Called when visiting {eventName} event.");
                writer.WriteLine($"    fn visit_{snakeName}(&mut self, payload: &{eventName}Payload) {{}}");
            }
            else
            {
                writer.WriteLine($"    /// Called when visiting {eventName} event (no payload).");
                writer.WriteLine($"    fn visit_{snakeName}(&mut self) {{}}");
            }
        }
        
        writer.WriteLine();
        writer.WriteLine("    /// Called when visiting an unknown event.");
        writer.WriteLine("    fn visit_unknown(&mut self, event_hash: i32, payload: &[u8]) {}");
        writer.WriteLine("}");
        writer.WriteLine();
        
        // Generate accept() method on StaticEventPayload
        writer.WriteLine("impl StaticEventPayload {");
        writer.WriteLine("    /// Accept a visitor, dispatching to the appropriate visit method.");
        writer.WriteLine("    pub fn accept<V: StaticEventVisitor>(&self, visitor: &mut V) {");
        writer.WriteLine("        match self {");
        
        foreach (var ev in staticEvents)
        {
            var eventName = GetUniqueEventName(ev.Key, ev.Value.MethodInfo);
            var snakeName = StringToSnakeCase(eventName);
            var hasPayload = eventsWithPayloads.Contains(eventName);
            
            if (hasPayload)
            {
                writer.WriteLine($"            Self::{eventName}(payload) => visitor.visit_{snakeName}(payload),");
            }
            else
            {
                writer.WriteLine($"            Self::{eventName} => visitor.visit_{snakeName}(),");
            }
        }
        
        writer.WriteLine("            Self::Unknown { event_hash, payload } => visitor.visit_unknown(*event_hash, payload),");
        writer.WriteLine("        }");
        writer.WriteLine("    }");
        
        writer.WriteLine();
        writer.WriteLine("    /// Get the event type for this payload.");
        writer.WriteLine("    pub fn event_type(&self) -> Option<StaticEventType> {");
        writer.WriteLine("        match self {");
        foreach (var ev in staticEvents)
        {
            var eventName = GetUniqueEventName(ev.Key, ev.Value.MethodInfo);
            var hasPayload = eventsWithPayloads.Contains(eventName);
            if (hasPayload)
            {
                writer.WriteLine($"            Self::{eventName}(_) => Some(StaticEventType::{eventName}),");
            }
            else
            {
                writer.WriteLine($"            Self::{eventName} => Some(StaticEventType::{eventName}),");
            }
        }
        writer.WriteLine("            Self::Unknown { .. } => None,");
        writer.WriteLine("        }");
        writer.WriteLine("    }");
        writer.WriteLine("}");
    }
    
    private void GenerateInstanceEventsFile(
        string identityDir,
        List<(int TypeHash, string FullTypeName, string ShortName)> typesWithEvents,
        Func<(int TypeHash, string FullTypeName, string ShortName), string> GetUniqueTypeName)
    {
        // Create instance_events subdirectory
        var instanceEventsDir = Path.Combine(identityDir, "instance_events");
        Directory.CreateDirectory(instanceEventsDir);
        
        // Group types by category (3rd namespace segment)
        // Maps: category -> list of (moduleName, uniqueTypeName, typeRawInfo, events)
        var categorizedTypes = new Dictionary<string, List<(string ModuleName, string UniqueTypeName, MySynchronizedTypeInfo TypeInfo, List<(uint, MethodInfo)> Events)>>();
        
        for (int typeIndex = 0; typeIndex < _typeIndexedList.Count; typeIndex++)
        {
            var typeRawInfo = _typeIndexedList[typeIndex];
            
            var events = GetEventTableMapping(typeRawInfo.EventTable)
                .OrderBy(e => e.Key)
                .Select(e => (e.Key, e.Value.MethodInfo))
                .ToList();
            
            if (events.Count == 0) continue;
            
            var shortName = GetShortTypeName(typeRawInfo.FullTypeName);
            var typeInfoTuple = (typeRawInfo.TypeHash, typeRawInfo.FullTypeName, shortName);
            var uniqueTypeName = GetUniqueTypeName(typeInfoTuple);
            var moduleName = StringToSnakeCase(uniqueTypeName);
            var category = GetCategoryFromNamespace(typeRawInfo.FullTypeName);
            
            if (!categorizedTypes.ContainsKey(category))
                categorizedTypes[category] = new List<(string, string, MySynchronizedTypeInfo, List<(uint, MethodInfo)>)>();
            
            categorizedTypes[category].Add((moduleName, uniqueTypeName, typeRawInfo, events));
        }
        
        // Generate per-category subdirectories and files
        foreach (var (category, types) in categorizedTypes.OrderBy(kv => kv.Key))
        {
            var categoryDir = Path.Combine(instanceEventsDir, category);
            Directory.CreateDirectory(categoryDir);
            
            // Generate per-type files within this category
            foreach (var (moduleName, uniqueTypeName, typeRawInfo, events) in types)
            {
                GenerateInstanceEventTypeFile(categoryDir, moduleName, uniqueTypeName, typeRawInfo, events);
            }
            
            // Generate category mod.rs
            GenerateCategoryModFile(categoryDir, category, types.Select(t => t.ModuleName).ToList());
        }
        
        // Generate top-level instance_events/mod.rs
        GenerateInstanceEventsRootModFile(instanceEventsDir, categorizedTypes.Keys.ToList());
    }
    
    /// <summary>
    /// Extract category from namespace (3rd segment, normalized).
    /// E.g., "Sandbox.Game.Entities.Cube.MyTerminalBlock" -> "entities"
    /// </summary>
    private static string GetCategoryFromNamespace(string fullTypeName)
    {
        var parts = fullTypeName.Split('.');
        if (parts.Length >= 3)
        {
            var segment = parts[2].ToLowerInvariant();
            return segment switch
            {
                "entities" => "entities",
                "entitycomponents" => "components",
                "components" => "components",
                "weapons" => "weapons",
                _ => "other"
            };
        }
        return "other";
    }
    
    private void GenerateInstanceEventTypeFile(
        string categoryDir,
        string moduleName,
        string uniqueTypeName,
        MySynchronizedTypeInfo typeRawInfo,
        List<(uint, MethodInfo)> events)
    {
        var filename = Path.Combine(categoryDir, $"{moduleName}.rs");
        using var writer = new StreamWriter(filename);
        
        writer.WriteLine($"//! Instance events for {typeRawInfo.FullTypeName}.");
        writer.WriteLine("//!");
        writer.WriteLine("//! Auto-generated by codegen. Do not edit manually.");
        writer.WriteLine();
        writer.WriteLine("#![allow(unused_imports)]");
        writer.WriteLine();
        writer.WriteLine("use deku::prelude::*;");
        writer.WriteLine("use space_engineers_compat::{BitAligned, BitBool, Nullable, VarMap, VarVec, VarBytes, VarString};");
        writer.WriteLine("use space_engineers_compat::math::Vector3F;");
        writer.WriteLine("use space_engineers_sys::math::Vector3D;");
        writer.WriteLine();
        
        // Track method name collisions within this type
        var instanceMethodNameCounts = events
            .GroupBy(e => GetMethodName(e.Item2))
            .ToDictionary(g => g.Key, g => g.Count());
        
        string GetUniqueInstanceEventName((uint, MethodInfo MethodInfo) ev)
        {
            var baseName = GetMethodName(ev.MethodInfo);
            return instanceMethodNameCounts[baseName] > 1 ? $"{baseName}_{ev.Item1}" : baseName;
        }
        
        // Generate payload structs for this type's instance events  
        var instanceEventsWithPayloads = new List<(string EventName, int EventHash, List<(string Name, string RustType)> Args)>();
        foreach (var ev in events)
        {
            var eventName = GetUniqueInstanceEventName(ev);
            var eventHash = ComputeEventHash(ev.Item2);
            var args = GetMethodArgs(ev.Item2);
            
            if (args.Count > 0)
            {
                instanceEventsWithPayloads.Add((eventName, eventHash, args));
                
                writer.WriteLine($"/// Payload for {uniqueTypeName}::{eventName} instance event.");
                writer.WriteLine("#[derive(Debug, Clone, PartialEq, DekuRead, DekuWrite)]");
                writer.WriteLine($"pub struct {uniqueTypeName}_{eventName}Payload {{");
                foreach (var (name, rustType) in args)
                {
                    writer.WriteLine($"    pub {name}: {rustType},");
                }
                writer.WriteLine("}");
                writer.WriteLine();
            }
        }
        
        // Generate per-type instance event identity enum
        writer.WriteLine($"/// Stable instance event identities for {typeRawInfo.FullTypeName}.");
        writer.WriteLine("///");
        writer.WriteLine("/// Use `Version` to convert to/from version-specific event IDs.");
        writer.WriteLine("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]");
        writer.WriteLine($"pub enum {uniqueTypeName}InstanceEvent {{");
        
        foreach (var ev in events)
        {
            var eventName = GetUniqueInstanceEventName(ev);
            var eventHash = ComputeEventHash(ev.Item2);
            writer.WriteLine($"    /// Hash: {eventHash}");
            writer.WriteLine($"    {eventName},");
        }
        
        writer.WriteLine("}");
        writer.WriteLine();
        
        // Generate event_hash() and from_hash() for this type's instance events
        writer.WriteLine($"impl {uniqueTypeName}InstanceEvent {{");
        writer.WriteLine("    /// Get the stable hash of this instance event.");
        writer.WriteLine("    pub const fn event_hash(&self) -> i32 {");
        writer.WriteLine("        match self {");
        foreach (var ev in events)
        {
            var eventName = GetUniqueInstanceEventName(ev);
            var eventHash = ComputeEventHash(ev.Item2);
            writer.WriteLine($"            Self::{eventName} => {eventHash},");
        }
        writer.WriteLine("        }");
        writer.WriteLine("    }");
        writer.WriteLine();
        
        writer.WriteLine("    /// Look up from event hash. Returns None for unknown events.");
        writer.WriteLine("    pub fn from_hash(hash: i32) -> Option<Self> {");
        writer.WriteLine("        match hash {");
        foreach (var ev in events)
        {
            var eventName = GetUniqueInstanceEventName(ev);
            var eventHash = ComputeEventHash(ev.Item2);
            writer.WriteLine($"            {eventHash} => Some(Self::{eventName}),");
        }
        writer.WriteLine("            _ => None,");
        writer.WriteLine("        }");
        writer.WriteLine("    }");
        writer.WriteLine("}");
        writer.WriteLine();
        
        // Generate InstanceEventPayload dispatch enum for this type
        writer.WriteLine($"/// Parsed instance event payload for {typeRawInfo.FullTypeName}.");
        writer.WriteLine("///");
        writer.WriteLine($"/// Use `{uniqueTypeName}InstanceEvent::parse_payload()` to parse raw bytes.");
        writer.WriteLine("#[derive(Debug, Clone, PartialEq)]");
        writer.WriteLine($"pub enum {uniqueTypeName}InstanceEventPayload {{");
        
        foreach (var ev in events)
        {
            var eventName = GetUniqueInstanceEventName(ev);
            var hasPayload = instanceEventsWithPayloads.Any(e => e.EventName == eventName);
            
            if (hasPayload)
            {
                writer.WriteLine($"    /// {eventName} event payload");
                writer.WriteLine($"    {eventName}({uniqueTypeName}_{eventName}Payload),");
            }
            else
            {
                writer.WriteLine($"    /// {eventName} event (no payload)");
                writer.WriteLine($"    {eventName},");
            }
        }
        
        writer.WriteLine("    /// Unknown event (raw bytes preserved)");
        writer.WriteLine("    Unknown { event_hash: i32, payload: Vec<u8> },");
        writer.WriteLine("}");
        writer.WriteLine();
        
        // Add parse_payload method to the instance event type
        writer.WriteLine("#[allow(unused_variables)]");
        writer.WriteLine($"impl {uniqueTypeName}InstanceEvent {{");
        writer.WriteLine("    /// Parse the payload bytes for this instance event type.");
        writer.WriteLine("    ///");
        writer.WriteLine("    /// # Errors");
        writer.WriteLine("    ///");
        writer.WriteLine("    /// Returns `Err` if the payload bytes don't match the expected format.");
        writer.WriteLine($"    pub fn parse_payload(&self, bytes: &[u8]) -> Result<{uniqueTypeName}InstanceEventPayload, deku::DekuError> {{");
        writer.WriteLine("        match self {");
        
        foreach (var ev in events)
        {
            var eventName = GetUniqueInstanceEventName(ev);
            var hasPayload = instanceEventsWithPayloads.Any(e => e.EventName == eventName);
            
            if (hasPayload)
            {
                writer.WriteLine($"            Self::{eventName} => {{");
                writer.WriteLine($"                let (_, payload) = {uniqueTypeName}_{eventName}Payload::from_bytes((bytes, 0))?;");
                writer.WriteLine($"                Ok({uniqueTypeName}InstanceEventPayload::{eventName}(payload))");
                writer.WriteLine("            }");
            }
            else
            {
                writer.WriteLine($"            Self::{eventName} => Ok({uniqueTypeName}InstanceEventPayload::{eventName}),");
            }
        }
        
        writer.WriteLine("        }");
        writer.WriteLine("    }");
        writer.WriteLine("}");
        writer.WriteLine();
        
        // Helper to parse from raw event hash + bytes for this type
        writer.WriteLine($"/// Parse an instance event payload for {uniqueTypeName} from raw event hash and bytes.");
        writer.WriteLine($"pub fn parse_{StringToSnakeCase(uniqueTypeName)}_instance_event(event_hash: i32, payload_bytes: &[u8]) -> Result<{uniqueTypeName}InstanceEventPayload, deku::DekuError> {{");
        writer.WriteLine($"    match {uniqueTypeName}InstanceEvent::from_hash(event_hash) {{");
        writer.WriteLine("        Some(event_type) => event_type.parse_payload(payload_bytes),");
        writer.WriteLine($"        None => Ok({uniqueTypeName}InstanceEventPayload::Unknown {{");
        writer.WriteLine("            event_hash,");
        writer.WriteLine("            payload: payload_bytes.to_vec(),");
        writer.WriteLine("        }),");
        writer.WriteLine("    }");
        writer.WriteLine("}");
        writer.WriteLine();
        
        // Generate visitor trait for this type's instance events
        writer.WriteLine($"/// Visitor trait for {uniqueTypeName} instance events.");
        writer.WriteLine("///");
        writer.WriteLine("/// Implement only the methods you care about - all have default empty implementations.");
        writer.WriteLine("#[allow(unused_variables)]");
        writer.WriteLine($"pub trait {uniqueTypeName}InstanceEventVisitor {{");
        
        foreach (var ev in events)
        {
            var eventName = GetUniqueInstanceEventName(ev);
            var snakeName = StringToSnakeCase(eventName);
            var hasPayload = instanceEventsWithPayloads.Any(e => e.EventName == eventName);
            
            if (hasPayload)
            {
                writer.WriteLine($"    /// Called when visiting {eventName} event.");
                writer.WriteLine($"    fn visit_{snakeName}(&mut self, payload: &{uniqueTypeName}_{eventName}Payload) {{}}");
            }
            else
            {
                writer.WriteLine($"    /// Called when visiting {eventName} event (no payload).");
                writer.WriteLine($"    fn visit_{snakeName}(&mut self) {{}}");
            }
        }
        
        writer.WriteLine();
        writer.WriteLine("    /// Called when visiting an unknown event.");
        writer.WriteLine("    fn visit_unknown(&mut self, event_hash: i32, payload: &[u8]) {}");
        writer.WriteLine("}");
        writer.WriteLine();
        
        // Generate accept() method on instance event payload
        writer.WriteLine($"impl {uniqueTypeName}InstanceEventPayload {{");
        writer.WriteLine("    /// Accept a visitor, dispatching to the appropriate visit method.");
        writer.WriteLine($"    pub fn accept<V: {uniqueTypeName}InstanceEventVisitor>(&self, visitor: &mut V) {{");
        writer.WriteLine("        match self {");
        
        foreach (var ev in events)
        {
            var eventName = GetUniqueInstanceEventName(ev);
            var snakeName = StringToSnakeCase(eventName);
            var hasPayload = instanceEventsWithPayloads.Any(e => e.EventName == eventName);
            
            if (hasPayload)
            {
                writer.WriteLine($"            Self::{eventName}(payload) => visitor.visit_{snakeName}(payload),");
            }
            else
            {
                writer.WriteLine($"            Self::{eventName} => visitor.visit_{snakeName}(),");
            }
        }
        
        writer.WriteLine("            Self::Unknown { event_hash, payload } => visitor.visit_unknown(*event_hash, payload),");
        writer.WriteLine("        }");
        writer.WriteLine("    }");
        writer.WriteLine();
        
        writer.WriteLine("    /// Get the instance event type for this payload.");
        writer.WriteLine($"    pub fn event_type(&self) -> Option<{uniqueTypeName}InstanceEvent> {{");
        writer.WriteLine("        match self {");
        foreach (var ev in events)
        {
            var eventName = GetUniqueInstanceEventName(ev);
            var hasPayload = instanceEventsWithPayloads.Any(e => e.EventName == eventName);
            if (hasPayload)
            {
                writer.WriteLine($"            Self::{eventName}(_) => Some({uniqueTypeName}InstanceEvent::{eventName}),");
            }
            else
            {
                writer.WriteLine($"            Self::{eventName} => Some({uniqueTypeName}InstanceEvent::{eventName}),");
            }
        }
        writer.WriteLine("            Self::Unknown { .. } => None,");
        writer.WriteLine("        }");
        writer.WriteLine("    }");
        writer.WriteLine("}");
    }
    
    private void GenerateCategoryModFile(string categoryDir, string category, List<string> moduleNames)
    {
        var modFilename = Path.Combine(categoryDir, "mod.rs");
        using var writer = new StreamWriter(modFilename);
        
        writer.WriteLine($"//! Instance events for {category} types.");
        writer.WriteLine("//!");
        writer.WriteLine("//! Auto-generated by codegen. Do not edit manually.");
        writer.WriteLine();
        
        foreach (var moduleName in moduleNames.OrderBy(m => m))
        {
            writer.WriteLine($"mod {moduleName};");
        }
        
        writer.WriteLine();
        
        foreach (var moduleName in moduleNames.OrderBy(m => m))
        {
            writer.WriteLine($"pub use {moduleName}::*;");
        }
    }
    
    private void GenerateInstanceEventsRootModFile(string instanceEventsDir, List<string> categories)
    {
        var modFilename = Path.Combine(instanceEventsDir, "mod.rs");
        using var writer = new StreamWriter(modFilename);
        
        writer.WriteLine("//! Instance event identities and payloads.");
        writer.WriteLine("//!");
        writer.WriteLine("//! Organized into per-category modules to improve compile times.");
        writer.WriteLine("//!");
        writer.WriteLine("//! Auto-generated by codegen. Do not edit manually.");
        writer.WriteLine();
        
        foreach (var category in categories.OrderBy(c => c))
        {
            writer.WriteLine($"pub mod {category};");
        }
        
        writer.WriteLine();
        
        // Re-export everything for backwards compatibility
        foreach (var category in categories.OrderBy(c => c))
        {
            writer.WriteLine($"pub use {category}::*;");
        }
    }
    
    /// <summary>
    /// Compute a stable hash for a method identity (doesn't change between versions).
    /// Uses FNV-1a hash of the method signature.
    /// </summary>
    private static int ComputeEventHash(MethodInfo method)
    {
        var signature = $"{method.DeclaringType?.FullName}.{method.Name}";
        foreach (var param in method.GetParameters())
        {
            signature += $"_{param.ParameterType.FullName}";
        }
        return ComputeFnv1aHash(signature);
    }
    
    /// <summary>
    /// FNV-1a hash implementation matching the game's MyUtils.GetHash.
    /// </summary>
    private static int ComputeFnv1aHash(string str)
    {
        const int FnvOffsetBasis = unchecked((int)0x811c9dc5);
        const int FnvPrime = 16777619;
        
        int hash = FnvOffsetBasis;
        int i = 0;
        
        while (i < str.Length - 1)
        {
            int combined = ((int)str[i] << 16) + str[i + 1];
            hash ^= combined;
            hash *= FnvPrime;
            i += 2;
        }
        
        if ((str.Length & 1) != 0)
        {
            hash ^= str[i];
            hash *= FnvPrime;
        }
        
        return hash;
    }
}

