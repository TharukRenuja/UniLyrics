# Getting started

[Documentation home](README.md)

## Choose your SDK

Use the [installation guide](INSTALLATION.md) for local package paths and linking. JavaScript and TypeScript run the Rust engine in WebAssembly. Python, Go, Java/Kotlin, Swift and C/C++ call a native library inside your process. Rust uses the core directly. Consumers of prebuilt engines do not need a Rust installation.

Check [platform requirements](PLATFORMS.md) before choosing a native binary.

## Parse your first lyrics

After installing the local npm package, this complete Node example initializes the engine and parses an explicit LRC format:

```javascript
import {readFile} from "node:fs/promises";
import {initialize} from "unilyrics";

const sdk = await initialize(
  await readFile(new URL(import.meta.resolve("unilyrics/core.wasm")))
);
const {document, diagnostics} = sdk.parse("[00:01.25]Hello", "lrc");
console.log(document.cues[0].start_us); // 1250000 microseconds
console.log(document.cues[0].segments[0].text); // Hello
console.log(diagnostics);
```

Initialize once and reuse the SDK. Parsing, conversion and model operations are synchronous after initialization. For browsers, supply the packaged WASM bytes from your asset pipeline; see [language guides](LANGUAGES.md#javascript-and-typescript).

## Convert and handle losses

```javascript
const converted = sdk.export(document, "plain", "compatible");
console.log(converted.output); // Hello
console.log(converted.losses); // Includes removed timing
```

Check both parse diagnostics and conversion losses. `strict` policy rejects a conversion when information would be lost. `compatible` returns supported output and describes losses; it does not bypass requirements such as subtitle timing. To save a simplified format and later recover the original canonical document, use a [preservation bundle](PRESERVATION.md).

## Use the result in your app

Store the document with the track, convert the playback clock to microseconds, and select active cues/segments at each update. The SDK does not run a playback clock or draw highlights. Follow [player integration](PLAYER_INTEGRATION.md) for missing end times, overlapping vocals and seek handling.

Other languages use the same model and semantics. See [language guides](LANGUAGES.md), [data model](DATA_MODEL.md), and [API workflows](API.md).
