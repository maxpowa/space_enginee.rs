namespace StandaloneExtractor;

/// <summary>
/// Holds configuration parsed from CLI arguments for use by the extractor plugin.
/// </summary>
public static class CodegenConfig
{
    /// <summary>
    /// Resolved path to the Space Engineers Bin64 directory.
    /// </summary>
    public static string Bin64Path { get; set; } = null!;

    /// <summary>
    /// If set, .rs files are written here instead of the default SEProtoExtractor directory.
    /// </summary>
    public static string? RustOutputPath { get; set; }
}
