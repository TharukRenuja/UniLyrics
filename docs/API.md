# API workflows

[Documentation home](README.md)

Operations use the same Rust semantics across SDKs. Names below use the JS API; see [language guides](LANGUAGES.md) and the linked detailed guides for native names.

| Operation | Input | Result |
|---|---|---|
| `detect` | Lyrics text | Built-in format identifier |
| `parse` | Text and optional format | Document and diagnostics |
| `validate` | Canonical document | Success or structured error |
| `inspect` | Canonical document | Available features |
| `normalize` | Document and signed offset in microseconds | New document and diagnostics |
| `project` | Document and profile options | New document and removal diagnostics |
| `export` | Document, target format and policy | Text and losses |
| `exportProfile` | Document, format, options and policy | Text and combined profile/format losses |
| `exportPreserved` | Document, target format and optional profile | Text, losses and sidecar |
| `restorePreserved` | Exact text, format and sidecar | Original canonical document and diagnostics |

## Parse and detect

```javascript
const explicit = sdk.parse(text, "lrc");
const detectedFormat = sdk.detect(text);
const automatic = sdk.parse(text); // Detects and parses
```

Prefer an explicit format when your provider already declares it. Detection is based on supported syntax, not provider identity or filename. Plain text is a fallback; successful parsing does not establish that every proprietary feature was understood. Check diagnostics and [format limits](FORMATS.md).

## Validate and inspect

```javascript
sdk.validate(document);
const features = sdk.inspect(document);
if (features.segment_timing) {
  // Offer timed segment highlighting for segments with explicit boundaries.
}
```

Inspection reports features present in the document. It does not guarantee every cue or segment has timing. Validation checks model constraints; it does not assess whether lyrics match the recording.

## Correct a known offset

```javascript
const shifted = sdk.normalize(document, 250000); // 250 ms later
```

This returns a new document. Missing times stay missing. A range error rejects the whole shift. Retained opaque attributes and offset metadata are not rewritten; review diagnostics. See [normalization](NORMALIZATION.md).

## Export deliberately

```javascript
const exact = sdk.export(document, "json", "strict");
const simpler = sdk.export(document, "lrc", "compatible");
const projected = sdk.exportProfile(document, "lrc", {profile: "line"}, "compatible");
```

`strict` rejects reported loss. `compatible` reports it. Profiles can intentionally remove features before conversion; profile and target losses are both reported. Neither policy invents missing ends required by SRT/WebVTT. See [profiles](PROFILES.md) and [conversion examples](COMPATIBILITY.md).

## Save and restore a preservation bundle

```javascript
const saved = sdk.exportPreserved(document, "lrc");
// Store saved.output and saved.sidecar together using your application's storage.
const restored = sdk.restorePreserved(saved.output, "lrc", saved.sidecar);
```

The sidecar contains the original canonical document, not original source bytes. An output edit invalidates the pair; restore does not merge edits. Treat a sidecar as untrusted input, not authenticated provenance. See [preservation](PRESERVATION.md).

## Add a custom format

Use an application-scoped registry with an `x-` format name. Your callbacks parse or export locally; the shared core validates returned documents and declared losses. Register only adapters your app needs. No global plugin installation or network discovery occurs. See [custom adapters](EXTENSIONS.md) for each language's callbacks and ownership contracts.

## Raw protocol

Managed helpers unwrap success results and surface failures using the language's error mechanism. C/C++ return the JSON envelope. For custom integrations:

```json
{"op":"parse","input":"[00:01.25]Hello","format":"lrc"}
```

Responses contain either `{"ok":true,"result":...}` or `{"ok":false,"error":{"code":"...","message":"..."}}`. See the [wire contract](../spec/README.md) for limits and ownership.

## .NET naming

The C# binding exposes `Parse`, `Detect`, `Export`, `Validate`, `Inspect`, `Normalize`, `Project`, `ExportProfile`, `ExportPreserved` and `RestorePreserved` on an owned `Lyrics` instance. Methods return typed records; `Process` provides lower-level JSON access. Errors from the shared engine become `LyricsException` with a `Code` property. Caller serialization and native loading failures retain ordinary .NET exception types. See [C# setup](INSTALLATION.md#c-and-net).
