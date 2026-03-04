using Microsoft.Win32;

namespace StandaloneExtractor.Launcher;

internal class Folder
{
    private const string seLauncher = "SpaceEngineers.exe";
    private static readonly HashSet<string> seFiles =
    [
        seLauncher,
        "SpaceEngineers.Game.dll",
        "VRage.dll",
        "Sandbox.Game.dll",
    ];

    public static bool IsBin64(string path)
    {
        if (!Directory.Exists(path))
            return false;

        foreach (string file in seFiles)
            if (!File.Exists(Path.Combine(path, file)))
                return false;

        return true;
    }

    /// <summary>
    /// Resolves the SE Bin64 path. Tries CWD first, then the Steam registry key.
    /// </summary>
    public static string GetBin64()
    {
        // 1. Try CWD
        var cwd = Directory.GetCurrentDirectory();
        if (IsBin64(cwd))
            return Path.GetFullPath(cwd);

        // 2. Try Steam registry (same key used in Directory.Build.props)
        var regPath = GetBin64FromRegistry();
        if (regPath != null && IsBin64(regPath))
            return Path.GetFullPath(regPath);

        throw new InvalidOperationException(
            "Could not find Space Engineers Bin64 folder. " +
            "Either run from the Bin64 directory, pass --bin64 <path>, or ensure SE is installed via Steam.");
    }

    private static string? GetBin64FromRegistry()
    {
        try
        {
            using var key = Registry.LocalMachine.OpenSubKey(
                @"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Steam App 244850");
            var installLocation = key?.GetValue("InstallLocation") as string;
            if (installLocation != null)
                return Path.Combine(installLocation, "Bin64");
        }
        catch
        {
            // Registry access may fail on non-Steam installs; fall through
        }

        return null;
    }
}

