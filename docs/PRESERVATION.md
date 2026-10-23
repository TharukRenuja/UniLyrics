# Preservation bundles

The initial preserve operation exports compatible target text alongside a versioned canonical snapshot. Rust exposes `export_preserved(document, format, options)` and `restore_preserved(output, format, sidecar)`. SDKs can use the shared protocol or the helpers described below. The helpers are available across the language bindings; see [platform requirements](PLATFORMS.md) for distribution availability.

```json
{"op":"export_preserved","document":{"version":1,"cues":[]},"format":"plain","options":{"profile":"full"}}
```

The result contains `output`, `losses`, and `sidecar`. Save output and sidecar together. Losses describe information unavailable in the target output alone; preservation does not erase that report. The sidecar always contains version 1, target format, exact output text, and the complete original canonical document. It also retains information intentionally removed by a selected profile. Omitted projection options select full. Target requirements still apply, including explicit timing required by subtitle formats.

```json
{"op":"restore_preserved","input":"<exact output>","format":"plain","sidecar":{}}
```

Replace the example sidecar with the complete returned sidecar. Restore checks version, exact output string, target format, and canonical model validity. A changed newline or other output edit yields `SIDECAR_MISMATCH`; an unsupported version yields `UNSUPPORTED_SIDECAR_VERSION`. Successful restore returns the original canonical document and a `SIDECAR_RESTORED` diagnostic. It does not merge edits.

This preserves canonical information, not the original source file bytes, styling, or unsupported source semantics that parsing never captured. The snapshot duplicates the exported text deliberately so pairing requires no checksum dependency. It is not a signature, trusted attribution, or proof against someone altering both files. Treat sidecars as untrusted documents and apply ordinary storage/access controls.

Generated bundles must fit the 1 MiB compact shared restore-request budget, including both copies of output, sidecar document and JSON encoding overhead. Too-large bundles fail with `INPUT_TOO_LARGE`, rather than returning a snapshot that the shared SDK protocol cannot restore. Language serializers can add whitespace or escaping. SDK helpers check the actual encoded restore request before returning a bundle. If serializer overhead exceeds the budget, export fails with `INPUT_TOO_LARGE`. This may reject different near-limit payloads in different languages, but returned helper bundles fit that helper's restore encoding. Raw protocol callers must check their own serialized request size. The returned result itself is not a restore request; applications should construct the documented request without adding extra fields.

Existing strict/compatible APIs and result models remain unchanged. Preservation uses its own result type so an older typed export reader cannot silently discard a sidecar. Custom registry callbacks are outside this first preservation operation.

## JavaScript/TypeScript and Python

JS/TS exposes `sdk.exportPreserved(document, format, options?)` and `sdk.restorePreserved(output, format, sidecar)`. TypeScript exports `PreservationSidecar` and `PreservedExportResult` types.

Python exposes `sdk.export_preserved(document, format, options=None)` and `sdk.restore_preserved(text, format, sidecar)`, returning ordinary typed dictionaries. `PreservationSidecar` and `PreservedExportResult` are public TypedDict models.

Both default to the full projection and preserve the complete original canonical snapshot even when a narrower profile is selected. Save the returned sidecar rather than building one from the projected output. Core validation and stale-pair checks apply identically through both helpers.

## Go

Go exposes `ExportPreserved(document, format, options)` returning `PreservedExportResult`, and `RestorePreserved(input, format, sidecar)` returning `ParseResult`. Use zero-value `ProjectionOptions{}` for full preservation, or the same explicit profile/custom options accepted by `Project`. `PreservationSidecar` is a JSON-serializable public struct. Save it together with the output string; restoration validates its version, canonical document, exact output and format.

## Swift

Swift exposes `UniLyrics.exportPreserved(_:format:options:)` and `UniLyrics.restorePreserved(_:format:sidecar:)` for typed and Foundation dictionary values. Typed methods use `PreservedExportResult` and `PreservationSidecar`, both Codable/Equatable/Sendable; options default to full. Encode the sidecar for storage, then decode it before restore. The core validates its version and canonical model; matching output/format are required even when restoring through dictionary helpers.

## Java and Kotlin

Java `UniLyrics` and Kotlin `Lyrics` expose `exportPreserved` and `restorePreserved` for typed documents and maps. Typed calls use shared `LyricsModel.PreservationSidecar` and `LyricsModel.PreservedExportResult` records; omitted options select full. Both records provide `toMap` and `fromMap` for application JSON storage. Load the native SDK before calling `PreservationSidecar.fromMap`: it validates the raw sidecar through the core before decoding, rejecting unknown fields and fractional or unsupported versions rather than silently normalizing them. Raw map restore is also available. The caller must retain output, format and the complete sidecar.

## C and C++

C exposes `unilyrics_export_preserved(document, length, format, options, options_length)` and `unilyrics_restore_preserved(output, length, format, sidecar, sidecar_length)`. Formats use the existing codes. Empty options `(NULL, 0)` select full; empty output may also be `(NULL, 0)`, but sidecar JSON must be non-null. Input buffers are borrowed for the call. Returned JSON envelopes are owned by the library and must be released exactly once with `unilyrics_free`.

C++ exposes `unilyrics::export_preserved(document_json, format, options_json="{}")` and `unilyrics::restore_preserved(output, format, sidecar_json)`. Both return owned `std::string` JSON envelopes, freeing native responses automatically. Use your JSON library to extract/save the sidecar object; restore takes that object, not the full export-result envelope. Canonical models remain JSON interfaces in C/C++.

## C# and .NET

`Lyrics.ExportPreserved(document, format, options)` returns output, losses and a typed sidecar. Save output and sidecar together, then call `RestorePreserved(output, format, sidecar)`. The binding checks the encoded restore-request budget before returning a bundle. See the [.NET guide](../bindings/dotnet/README.md).
