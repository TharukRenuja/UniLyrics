using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;
using UniLyrics;

Console.InputEncoding = Encoding.UTF8;
Console.OutputEncoding = Encoding.UTF8;
if (args.Length > 0 && args[0] == "--protocol")
{
    using var protocolEngine = new Lyrics(args[1] == "-" ? null : args[1]);
    string? line;
    while ((line = Console.ReadLine()) is not null)
    {
        try { Console.WriteLine(JsonSerializer.Serialize(new { ok = true, result = protocolEngine.Process(JsonDocument.Parse(line).RootElement) })); }
        catch (LyricsException error) { Console.WriteLine(JsonSerializer.Serialize(new { ok = false, error = new { code = error.Code, message = error.Message } })); }
    }
    return;
}
var root = args[0];
using var sdk = new Lyrics(args[1] == "-" ? null : args[1]);
var json = new JsonSerializerOptions { PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower };
json.Converters.Add(new JsonStringEnumConverter<Format>(JsonNamingPolicy.SnakeCaseLower, false));
void Check(bool value, string message) { if (!value) throw new Exception(message); }
void Error(string code, Action action)
{
    try { action(); throw new Exception("Expected " + code); }
    catch (LyricsException error) { Check(error.Code == code, $"Expected {code}, received {error.Code}"); }
}
bool Equal(Document a, Document b) => JsonNode.DeepEquals(JsonNode.Parse(sdk.Export(a, Format.Json, Policy.Strict).Output), JsonNode.Parse(sdk.Export(b, Format.Json, Policy.Strict).Output));
var fixtures = JsonDocument.Parse(File.ReadAllText(Path.Combine(root, "crates/unilyrics/tests/fixtures/conformance.json"))).RootElement;
int count = 0;
foreach (var fixture in fixtures.EnumerateArray())
{
    var input = fixture.TryGetProperty("input", out var text) ? text.GetString()! : fixture.GetProperty("document").GetRawText();
    var format = fixture.GetProperty("format").Deserialize<Format>(json);
    if (fixture.TryGetProperty("error", out var error)) { Error(error.GetString()!, () => sdk.Parse(input, format)); continue; }
    var parsed = sdk.Parse(input, format);
    var cues = parsed.Document.Cues;
    Check(cues.Select(c => string.Concat(c.Segments.Select(s => s.Text))).SequenceEqual(fixture.GetProperty("text").EnumerateArray().Select(s => s.GetString())), "Typed cue text");
    Check(cues.Select(c => c.StartUs).SequenceEqual(fixture.GetProperty("starts").EnumerateArray().Select(t => t.ValueKind == JsonValueKind.Null ? (ulong?)null : t.GetUInt64())), "Typed cue timing");
    Check(sdk.Validate(parsed.Document).Valid, "Typed validation");
    if (fixture.TryGetProperty("document", out var expected))
        Check(JsonNode.DeepEquals(JsonNode.Parse(sdk.Export(parsed.Document, Format.Json, Policy.Strict).Output), JsonNode.Parse(expected.GetRawText())), "Complete typed canonical round trip");
    var customRegistry = new Registry(sdk);
    customRegistry.Register("x-canonical", new Adapter {
        Parser = value => sdk.Parse(value, Format.Json),
        Exporter = (value, policy) => sdk.Export(value, Format.Json, Policy.Strict)
    });
    var exported = customRegistry.Export(parsed.Document, "x-canonical", Policy.Strict);
    Check(Equal(parsed.Document, customRegistry.Parse(exported.Output, "x-canonical").Document), "Rich custom canonical round trip");
    count++;
}
var document = sdk.Parse("[00:01]සිංහල 世界 👩🏽‍💻", Format.Lrc).Document;
Check(sdk.Detect("[00:01]Hello") == Format.Lrc, "Detect");
Check(sdk.Inspect(document).LineTiming, "Inspect");
var shifted = sdk.Normalize(document, -500_000).Document;
Check(shifted.Cues[0].StartUs == 500_000 && document.Cues[0].StartUs == 1_000_000, "Immutable signed normalization");
var plain = sdk.Export(document, Format.Plain);
Check(plain.Output.Contains("සිංහල 世界 👩🏽‍💻") && plain.Losses.Count > 0, "Unicode and conversion losses");
Error("LOSSY_CONVERSION", () => sdk.Export(document, Format.Plain, Policy.Strict));
Check(sdk.Project(document, new() { Profile = Profile.Plain }).Document.Cues[0].StartUs is null, "Projection");
Check(sdk.ExportProfile(document, Format.Plain, new() { Profile = Profile.Plain }).Losses.Count > 0, "Profile losses");
var preserved = sdk.ExportPreserved(document, Format.Plain);
Check(Equal(document, sdk.RestorePreserved(preserved.Output, Format.Plain, preserved.Sidecar).Document), "Preservation round trip");
try { sdk.RestorePreserved(preserved.Output + "changed", Format.Plain, preserved.Sidecar); throw new Exception("Stale sidecar accepted"); }
catch (LyricsException) { }
Error("INVALID_UTF8", () => sdk.Parse("\ud800", Format.Plain));
Error("INVALID_UTF8", () => sdk.Parse("\udc00", Format.Plain));
Error("INPUT_TOO_LARGE", () => sdk.Parse(new string('x', 1_048_577), Format.Plain));
var attributes = sdk.Parse("text", Format.Plain).Document;
attributes.Attributes["exact"] = JsonSerializer.SerializeToElement(ulong.MaxValue);
Check(sdk.Parse(sdk.Export(attributes, Format.Json, Policy.Strict).Output, Format.Json).Document.Attributes["exact"].GetUInt64() == ulong.MaxValue, "Exact attribute integer");
attributes.Attributes["\ud800"] = JsonSerializer.SerializeToElement(1);
Error("INVALID_UTF8", () => sdk.Export(attributes, Format.Json));
var registry = new Registry(sdk);
registry.Register("x-wrap", new Adapter
{
    Parser = input => sdk.Parse(input[5..], Format.Json),
    Exporter = (value, policy) => new ExportResult { Output = "WRAP:" + sdk.Export(value, Format.Json, Policy.Strict).Output, Losses = [] },
    Detector = input => input.StartsWith("WRAP:", StringComparison.Ordinal)
});
var wrapped = registry.Export(document, "x-wrap", Policy.Strict);
Check(registry.Detect(wrapped.Output) == "x-wrap" && Equal(document, registry.Parse(wrapped.Output).Document), "Custom adapter round trip");
Error("DUPLICATE_FORMAT", () => registry.Register("x-wrap", new()));
Error("INVALID_CUSTOM_FORMAT", () => registry.Register("x-invalid\n", new()));
registry.Register("x-lossy", new Adapter { Exporter = (value, policy) => new ExportResult { Output = "text", Losses = [] } });
Error("LOSSY_CONVERSION", () => registry.Export(document, "x-lossy", Policy.Strict));
registry.Register("x-throw", new Adapter { Parser = _ => throw new InvalidOperationException("callback failure") });
try { registry.Parse("text", "x-throw"); throw new Exception("Callback error lost"); }
catch (InvalidOperationException error) { Check(error.Message == "callback failure", "Callback propagation"); }
for (int i = 0; i < 1_000; i++) Check(sdk.Parse("[00:01]世界 🎵", Format.Lrc).Document.Cues.Count == 1, "Repeated native calls");
Parallel.For(0, 100, _ => Check(sdk.Validate(document).Valid, "Concurrent native calls"));
using (var disposed = new Lyrics(args[1] == "-" ? null : args[1]))
{
    disposed.Dispose();
    disposed.Dispose();
    try { disposed.Parse("text"); throw new Exception("Disposed engine accepted call"); }
    catch (ObjectDisposedException) { }
}
Console.WriteLine($".NET typed fixtures ({count} valid cases), Unicode, lifecycle, registry, profiles and preservation passed.");
