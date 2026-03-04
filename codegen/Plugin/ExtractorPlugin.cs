using Sandbox.Game;
using VRage.Game;
using VRage.Plugins;

namespace StandaloneExtractor;

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

        Console.WriteLine($"Extractor v{version}");
        Console.WriteLine($"  .rs output:        {rsOutputPath}");
        Console.WriteLine($"  .json diagnostics: {diagnosticsPath}");

        // Serialize replication event tables to JSON
        var replicationEvents = new ReplicationEvents(diagnosticsPath);
        replicationEvents.Serialize();

        // Generate Rust struct/enum definitions into a single types.rs module
        var allTypes = new List<Type>
        {
            typeof(MyCachedServerItem.MyServerData),
            typeof(MyObjectBuilder_World),
        };
        allTypes.AddRange(replicationEvents.GetAllTypes());
        RustStructGenerator.GenerateRustStructs(allTypes, rsOutputPath, "types.rs");

        Console.WriteLine("Code generation complete!");
        Environment.Exit(0);
    }

    public void Update()
    {
        // Nothing needed here
    }
}

