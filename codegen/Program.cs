using System.Reflection;
using System.Runtime.InteropServices;
using Sandbox.ModAPI;
using StandaloneExtractor.Launcher;
using Game = StandaloneExtractor.Launcher.Game;

namespace StandaloneExtractor;

public static class Program
{
    public static void Main(string[] args)
    {
        // Parse custom arguments, separating them from game arguments
        string? bin64Arg = null;
        string? outputArg = null;
        var gameArgs = new List<string>();

        for (int i = 0; i < args.Length; i++)
        {
            switch (args[i])
            {
                case "--bin64" when i + 1 < args.Length:
                    bin64Arg = args[++i];
                    break;
                case "--output" when i + 1 < args.Length:
                    outputArg = args[++i];
                    break;
                default:
                    gameArgs.Add(args[i]);
                    break;
            }
        }

        var bin64Dir = bin64Arg != null && Folder.IsBin64(bin64Arg)
            ? Path.GetFullPath(bin64Arg)
            : Folder.GetBin64();

        CodegenConfig.Bin64Path = bin64Dir;
        CodegenConfig.RustOutputPath = outputArg != null ? Path.GetFullPath(outputArg) : null;

        Console.WriteLine($"[Codegen] Bin64: {bin64Dir}");
        if (CodegenConfig.RustOutputPath != null)
            Console.WriteLine($"[Codegen] Output: {CodegenConfig.RustOutputPath}");

        // Get the Windows Desktop runtime directory for Windows Forms, WPF, etc.
        var runtimeDir = RuntimeEnvironment.GetRuntimeDirectory();

        AppDomain.CurrentDomain.AssemblyResolve += AssemblyResolver([bin64Dir, runtimeDir]);

        // Write steam_appid.txt so the Steamworks SDK thinks Steam already launched us.
        // Without this, Steam intercepts the process and shows a relaunch dialog.
        var steamAppIdFile = Path.Combine(bin64Dir, "steam_appid.txt");
        var steamAppIdExisted = File.Exists(steamAppIdFile);
        if (!steamAppIdExisted)
            File.WriteAllText(steamAppIdFile, "244850");

        try
        {
            SetupGame(bin64Dir);
            Game.StartSpaceEngineers(gameArgs.ToArray());
        }
        finally
        {
            // Clean up if we created it
            if (!steamAppIdExisted && File.Exists(steamAppIdFile))
                File.Delete(steamAppIdFile);
        }
    }
    
    private static void SetupGame(string bin64Dir)
    {
        // MyFileSystem.RootPath = Path.Combine(bin64Dir, "../");
        // MyFileSystem.ExePath = bin64Dir;
        
        Game.SetMainAssembly(Path.Combine(bin64Dir, "SpaceEngineers.exe"));
        Game.SetupMyFakes();
        
        Game.RegisterPlugin(new ExtractorPlugin());
    }
    
    private static ResolveEventHandler AssemblyResolver(string[] probeDirs)
    {
        return (sender, args) =>
        {
            string targetName = new AssemblyName(args.Name).Name;

            foreach (string probeDir in probeDirs)
            {
                string targetPath = Path.Combine(probeDir, targetName);

                if (File.Exists(targetPath + ".dll"))
                    return Assembly.LoadFrom(targetPath + ".dll");

                if (File.Exists(targetPath + ".exe"))
                    return Assembly.LoadFrom(targetPath + ".exe");
            }
            
            Console.WriteLine($"[AssemblyResolver] Could not resolve assembly: {args.Name} (Looked in: {string.Join(", ", probeDirs)})");

            return null;
        };
    }
}

