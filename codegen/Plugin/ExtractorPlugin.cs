using Sandbox.Game;
using VRage.Game;
using VRage.Plugins;

namespace StandaloneExtractor.Plugin;

public class ExtractorPlugin : IPlugin
{
    public void Dispose()
    {
        // Nothing needed here
    }

    public void Init(object gameInstance)
    {
        var version = MyPerGameSettings.BasicGameInfo.GameVersion.GetValueOrDefault();

        // JSON diagnostics always go to a versioned folder alongside the game
        var diagnosticsPath = Path.Combine(CodegenConfig.Bin64Path, "SEProtoExtractor", version.ToString());
        Directory.CreateDirectory(diagnosticsPath);

        // .rs output goes to the user-specified path, or falls back to the diagnostics folder
        var rsOutputPath = CodegenConfig.RustOutputPath ?? diagnosticsPath;
        Directory.CreateDirectory(rsOutputPath);
        
        // Compute transport crate output path (sibling to sys crate)
        var transportOutputPath = Path.Combine(Path.GetDirectoryName(Path.GetDirectoryName(rsOutputPath)!)!, "crate-transport", "src");
        Directory.CreateDirectory(transportOutputPath);

        Console.WriteLine($"Extractor v{version}");
        Console.WriteLine($"  sys types.rs output:       {rsOutputPath}");
        Console.WriteLine($"  transport events.rs output:{transportOutputPath}");
        Console.WriteLine($"  .json diagnostics:         {diagnosticsPath}");

        // Serialize replication event tables to JSON
        var replicationEvents = new ReplicationEvents(diagnosticsPath);
        replicationEvents.Serialize();
        
        // Generate version-specific schema for runtime lookup
        // Write to both diagnostics (for debugging) and transport crate (for embedding)
        replicationEvents.GenerateVersionSchema(diagnosticsPath, version);
        replicationEvents.GenerateVersionSchema(Path.Combine(transportOutputPath, "protocol", "version"), version, embedded: true);
        
        // Generate stable identity types (hash-based, doesn't change between versions)
        replicationEvents.GenerateRustIdentityTypes(transportOutputPath);

        // Collect replication event argument types (these need Deku derives for binary serialization)
        var dekuTypes = new HashSet<Type>(replicationEvents.GetAllTypes());

        // Generate Rust struct/enum definitions into a single types.rs module
        var allTypes = new List<Type>
        {
            typeof(MyCachedServerItem.MyServerData),
            typeof(MyObjectBuilder_World),
        };
        allTypes.AddRange(dekuTypes);
        RustStructGenerator.GenerateRustStructs(allTypes, rsOutputPath, "types.rs", dekuTypes);

        Console.WriteLine("Code generation complete!");
        Environment.Exit(0);
    }

    public void Update()
    {
        // Nothing needed here
    }
}

