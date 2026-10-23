# UniLyrics for .NET

A C# API for the shared Rust lyrics engine, targeting .NET 10. The managed library uses System.Text.Json and native interop without third-party managed dependencies. It can be consumed by C#, F# and other compatible .NET applications. No lyrics parsers are duplicated in C#.

## Use the SDK

```csharp
using UniLyrics;

using var lyrics = new Lyrics();
var parsed = lyrics.Parse("[00:01.25]Hello 世界 🎵", Format.Lrc);
Console.WriteLine(parsed.Document.Cues[0].StartUs); // 1250000
Console.WriteLine(parsed.Document.Cues[0].Segments[0].Text);
var converted = lyrics.Export(parsed.Document, Format.Plain);
Console.WriteLine(converted.Output);
Console.WriteLine(converted.Losses.Count);
```

Keep one engine instance for your app and dispose it after all operations finish. `Lyrics()` resolves a packaged native asset through .NET's native loader. For a separate engine, pass its absolute path to the constructor or set `UNILYRICS_LIBRARY`. Explicit paths override packaged resolution. OS/CPU mismatches and missing native dependencies surface as .NET load exceptions.

The binding provides `Parse`, `Detect`, `Export`, `Validate`, `Inspect`, `Normalize`, `Project`, `ExportProfile`, `ExportPreserved` and `RestorePreserved`. `Process` accepts JSON-compatible requests and returns an independently owned `JsonElement` for lower-level operations. `LyricsException.Code` contains shared core error codes; caller serialization errors and native loading failures retain their ordinary .NET exceptions.

Times are nullable `ulong` microseconds; offset arguments use signed `long`. The core enforces its safe-integer timing range. Attributes are `Dictionary<string, JsonElement>`, preserving JSON nulls, nested values and exact integer values. Models are records with init properties, but their lists/dictionaries are mutable; callers must synchronize concurrent modifications. Operations return new documents rather than shifting the input in place.

## Build from source

Install the permanent .NET 10 SDK and Rust toolchain. Fetch Cargo dependencies once, then build:

```sh
cargo build --locked --offline -p unilyrics-ffi
dotnet build bindings/dotnet/UniLyrics/UniLyrics.csproj -c Release
```

Reference that project from your app and supply the matching native engine. The C interface exposes process/free entry points; response cleanup happens in a `finally` block and module lifetime uses SafeHandle. Calls on a disposed engine fail; native calls through a single instance are serialized.

## Local NuGet package

On Linux x64, `python -B scripts/package_dotnet.py` builds a local `UniLyrics.0.1.0.nupkg` containing the managed assembly and Linux native engine. Add `--windows-native /absolute/path/to/unilyrics_ffi.dll` to also bundle a prebuilt Windows x64 engine. Outputs go to `target/packages-dotnet`. No registry upload occurs. The package must contain the native asset for your application's target runtime.

Install the package from that local directory with `dotnet add package UniLyrics --version 0.1.0 --source /absolute/path/to/packages-dotnet`. No .NET Framework, NativeAOT, trimming, mobile or Unity compatibility is claimed. Windows native execution must be verified on Windows; a bundled DLL alone does not establish it. Apple native packaging remains separate work.

## Custom formats

```csharp
var registry = new Registry(lyrics);
registry.Register("x-example", new Adapter {
    Parser = text => lyrics.Parse(text, Format.Plain),
    Detector = text => text.StartsWith("EXAMPLE:")
});
var result = registry.Parse("EXAMPLE: Hello", "x-example");
```

Callbacks are synchronous. The registry validates custom parse/export results using the core. Exporters receive an independent canonical document; strict custom exports require an equivalent reparsed document. Callback registration/lookups are synchronized and callbacks execute outside the registry lock. Callbacks may run concurrently and must protect captured state. Primary callback exceptions propagate; failed export reparsing becomes a conversion loss.

MIT licensed. Native runtime dependencies retain their licenses in the package's `licenses/` directory.
