# UniLyrics for JavaScript and TypeScript

A simple engine for parsing, validating, and converting lyrics. The JavaScript API runs the shared Rust engine in WebAssembly and includes TypeScript declarations.

```javascript
import { readFile } from "node:fs/promises";
import { initialize } from "unilyrics";
const sdk = await initialize(await readFile(new URL(import.meta.resolve("unilyrics/core.wasm"))));
const { document } = sdk.parse("[00:01.25]Hello", "lrc");
const { output, losses } = sdk.export(document, "plain", "compatible");
```

The loader accepts WASM bytes or a compiled `WebAssembly.Module`. Initialize once, then reuse the synchronous SDK methods. Formats include plain text, LRC, enhanced LRC, canonical JSON, SRT, WebVTT and the documented TTML profiles. Conversions return losses; strict policy rejects information loss. Content retrieval and rendering belong to your application.

Source and local-package setup are described in the [installation guide](https://github.com/TharukRenuja/UniLyrics/blob/main/docs/INSTALLATION.md). The API is experimental; this package has not been published to the public npm registry.
