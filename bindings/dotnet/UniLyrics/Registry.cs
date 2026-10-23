using System.Text.Json;
using System.Text.RegularExpressions;

namespace UniLyrics;

public sealed record Adapter
{
    public Func<string, ParseResult>? Parser { get; init; }
    public Func<Document, Policy, ExportResult>? Exporter { get; init; }
    public Func<string, bool>? Detector { get; init; }
}

public sealed class Registry(Lyrics engine)
{
    private readonly object gate = new();
    private readonly Dictionary<string, Adapter> adapters = new(StringComparer.Ordinal);

    public void Register(string name, Adapter adapter)
    {
        ArgumentNullException.ThrowIfNull(adapter);
        if (name is null || !Regex.IsMatch(name, @"\Ax-[a-z0-9_-]{1,62}\z", RegexOptions.CultureInvariant))
            throw new LyricsException("INVALID_CUSTOM_FORMAT", "Use a 3–64 character lowercase ASCII name in the x- namespace.");
        lock (gate)
            if (!adapters.TryAdd(name, adapter)) throw new LyricsException("DUPLICATE_FORMAT", "Format is already registered.");
    }

    private Adapter? Find(string name) { lock (gate) return adapters.GetValueOrDefault(name); }
    private static void CheckInput(string input)
    {
        ArgumentNullException.ThrowIfNull(input);
        try
        {
            if (Lyrics.Utf8.GetByteCount(input) > Lyrics.RequestLimit)
                throw new LyricsException("INPUT_TOO_LARGE", "Input exceeds 1 MiB.");
        }
        catch (System.Text.EncoderFallbackException error) { throw new LyricsException("INVALID_UTF8", error.Message); }
    }
    private static string Name(Format format) => JsonSerializer.SerializeToElement(format, Lyrics.Json).GetString()!;
    private static Format Builtin(string format)
    {
        foreach (var value in Enum.GetValues<Format>()) if (Name(value) == format) return value;
        throw new LyricsException("UNKNOWN_FORMAT", "Format is not registered.");
    }

    public string Detect(string input)
    {
        CheckInput(input);
        Format? builtin = null;
        LyricsException? failure = null;
        try { builtin = engine.Detect(input); }
        catch (LyricsException error) { failure = error; }
        KeyValuePair<string, Adapter>[] snapshot;
        lock (gate) snapshot = adapters.ToArray();
        var matches = snapshot.Where(entry => entry.Value.Detector?.Invoke(input) == true).Select(entry => entry.Key).ToArray();
        if (matches.Length > 1 || (matches.Length == 1 && builtin is not null && builtin != Format.Plain))
            throw new LyricsException("AMBIGUOUS_FORMAT", "Multiple formats match; choose explicitly.");
        if (matches.Length == 1) return matches[0];
        if (failure is not null) throw failure;
        return Name(builtin!.Value);
    }

    public ParseResult Parse(string input, string? format = null)
    {
        CheckInput(input);
        var name = format ?? Detect(input);
        var adapter = Find(name);
        if (adapter is null) return engine.Parse(input, Builtin(name));
        if (adapter.Parser is null) throw new LyricsException("UNSUPPORTED_OPERATION", "Custom format has no parser.");
        return engine.Call<ParseResult>(new { op = "validate_custom_parse", result = adapter.Parser(input) });
    }

    public ExportResult Export(Document document, string format, Policy policy = Policy.Compatible)
    {
        var adapter = Find(format);
        if (adapter is null) return engine.Export(document, Builtin(format), policy);
        if (!Enum.IsDefined(policy)) throw new LyricsException("INVALID_POLICY", "Unknown export policy.");
        var canonical = engine.Export(document, Format.Json, Policy.Strict).Output;
        var snapshot = JsonSerializer.Deserialize<Document>(canonical, Lyrics.Json)!;
        if (adapter.Exporter is null) throw new LyricsException("UNSUPPORTED_OPERATION", "Custom format has no exporter.");
        var result = engine.Call<ExportResult>(new { op = "validate_custom_export", result = adapter.Exporter(snapshot, policy) });
        Document? roundtrip = null;
        if (adapter.Parser is not null)
        {
            try { roundtrip = engine.Call<ParseResult>(new { op = "validate_custom_parse", result = adapter.Parser(result.Output) }).Document; }
            // Failed reparsing must become a core loss; primary callback failures still propagate.
            catch (Exception) { }
        }
        return engine.Call<ExportResult>(new { op = "verify_custom_export", document = JsonSerializer.Deserialize<Document>(canonical, Lyrics.Json), result, roundtrip, policy });
    }
}
