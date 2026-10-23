<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/unilyrics-white.svg">
    <img src="assets/unilyrics-black.svg" alt="" width="144">
  </picture>
</p>

<h1 align="center">UniLyrics</h1>
<p align="center"><strong>A simple engine for parsing, validating, and converting lyrics.</strong></p>
<p align="center"><a href="docs/GETTING_STARTED.md">Get started</a> · <a href="docs/LANGUAGES.md">Choose your language</a> · <a href="docs/README.md">Documentation</a> · <a href="examples/README.md">Examples</a></p>

Lyrics arrive as plain text, timestamped lines, timed words, subtitles and rich vocal documents. UniLyrics brings them into one structured model so your app can work with the content instead of maintaining a different parser for every format.

A shared Rust engine powers bindings for **11 languages**: Rust, JavaScript, TypeScript, Python, Go, Java, Kotlin, Swift, C, C++ and C#/.NET. Parsing happens locally. There is no lyrics service, account or server to run.

## Parse once. Work with the result.

After [installing the JavaScript package](docs/INSTALLATION.md#javascript-and-typescript):

```javascript
import { readFile } from "node:fs/promises";
import { initialize } from "unilyrics";

const lyrics = await initialize(
  await readFile(new URL(import.meta.resolve("unilyrics/core.wasm")))
);

const { document } = lyrics.parse("[00:01.25]Hello world", "lrc");

console.log(document.cues[0].start_us);          // 1250000
console.log(document.cues[0].segments[0].text);  // Hello world

const converted = lyrics.export(document, "plain", "compatible");
console.log(converted.output); // Hello world
console.log(converted.losses); // Reports the timing removed by plain text
```

The same model and conversion rules apply across the language bindings. Initialize the engine once, then reuse it for parsing, inspection and conversion.

## Built for more than lines of text

- **Line and word timing:** Work with explicit timestamps in integer microseconds, including overlapping cues and vocal segments.
- **Rich lyrics:** Represent languages, singers, lead/background relationships, translations and romanization when the source supplies them.
- **Conversions you can inspect:** Compatible conversions report what was removed; strict conversions reject information loss.
- **Useful editing operations:** Shift timestamps, project feature profiles and preserve a canonical snapshot alongside simplified output.
- **Your own formats:** Register custom adapters while using the same model validation and conversion policies.

UniLyrics handles lyrics data. Your app chooses the source, runs the playback clock and renders the lyrics. See [player integration](docs/PLAYER_INTEGRATION.md) for using the model in a music player.

## Formats

| Format | Identifier | Use |
|---|---|---|
| Plain text | `plain` | Untimed lyrics |
| LRC | `lrc` | Line-synchronized lyrics |
| Enhanced LRC | `enhanced_lrc` | Inline word timing |
| Canonical JSON | `json` | The complete UniLyrics document model |
| SRT | `srt` | Subtitle cues |
| WebVTT | `webvtt` | Web subtitle cues and supported inline timing |
| TTML | `ttml` | The documented timed-text profile |
| Lyrics TTML | `ttml_lyrics` | Supported vocal and auxiliary lyric structures |

Support follows [documented format profiles](docs/FORMATS.md), rather than every extension used by every provider. Missing timing and word boundaries are not invented. See [conversion behavior](docs/COMPATIBILITY.md) before choosing an output format.

## Use it in your language

| Runtime | Integration |
|---|---|
| Rust | Call the core directly |
| JavaScript / TypeScript | Load the WebAssembly engine in Node or a browser |
| Python | Native engine through ctypes |
| Java / Kotlin | JVM facade and JNI; Android AAR |
| Go | Typed API through cgo |
| Swift | SwiftPM wrapper with Codable models |
| C# / .NET | Typed .NET 10 API and native engine through interop |
| C / C++ | C ABI and C++17 ownership wrapper |

Start with [installation](docs/INSTALLATION.md), follow your [language guide](docs/LANGUAGES.md), or open a [complete example](examples/README.md).

## C# and .NET

Use the same engine from a .NET 10 application with typed documents, structured errors and native memory cleanup:

```csharp
using UniLyrics;

using var lyrics = new Lyrics();
var parsed = lyrics.Parse("[00:01.25]Hello world", Format.Lrc);
var converted = lyrics.Export(parsed.Document, Format.Plain);
Console.WriteLine(converted.Output); // Hello world
```

Install a locally built NuGet package or reference the source project and provide the matching native engine. See the [.NET guide](bindings/dotnet/README.md) for setup and custom adapters. Packages are not published on NuGet yet; Windows native execution remains unverified.

## Availability

UniLyrics is **experimental**. Build packages from this repository; public registry installation and a stable API are not available yet. Linux is the current complete package path, Android supports API 29+, Windows packaging is preliminary, and macOS/iOS distribution is still in development. See [platform requirements](docs/PLATFORMS.md) before integrating.

## Build from source

Install Rust and fetch the pinned dependencies, then build the shared engine:

```sh
cargo fetch --locked
cargo build --locked --release -p unilyrics-ffi
```

You only need the toolchains for the bindings you use. The [installation guide](docs/INSTALLATION.md) covers WASM, native linking and package builds.

## License

[MIT](LICENSE) © 2026 [Tharuk Renuja](https://github.com/TharukRenuja). Dependencies retain their own [licenses and notices](THIRD_PARTY_NOTICES.md).
