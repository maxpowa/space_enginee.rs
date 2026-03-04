using System.Reflection;
using Sandbox.Engine.Utils;
using SpaceEngineers;
using VRage.FileSystem;
using VRage.Plugins;

namespace StandaloneExtractor.Launcher;

internal static class Game
{
    public static void RegisterPlugin(IPlugin plugin)
    {
        var mPluginsField = typeof(MyPlugins).GetField(
            "m_plugins",
            BindingFlags.Static | BindingFlags.NonPublic
        );
        var mPlugins = (List<IPlugin>)mPluginsField?.GetValue(null)!;
        mPlugins.Add(plugin);
    }

    public static void SetMainAssembly(string assemblyPath)
    {
        var asmFolder = new FileInfo(assemblyPath).DirectoryName;
        var seRoot = new FileInfo(asmFolder ?? throw new InvalidOperationException("Could not set main assembly path")).Directory!.FullName;

        MyFileSystem.ExePath = asmFolder;
        MyFileSystem.RootPath = seRoot;

        Environment.CurrentDirectory = asmFolder;
    }

    public static void SetupMyFakes()
    {
        typeof(MyFakes).TypeInitializer?.Invoke(null, null);
        MyFakes.ENABLE_F12_MENU = false;

        // Note SpaceEngineers internally prioritises -nosplash over ENABLE_SPLASHSCREEN
        // (therefore SplashType.Native and SplashType.None are mutually exclusive)
        MyFakes.ENABLE_SPLASHSCREEN = false;
    }

    public static void StartSpaceEngineers(string[] args) => MyProgram.Main(args);
}

