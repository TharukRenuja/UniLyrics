# Experimental core contract, phase 1

The shared core currently supports `plain`, `lrc`, `enhanced_lrc`, [`srt`](../docs/SRT.md), [`webvtt`](../docs/WEBVTT.md), `json`, and experimental [`ttml`/`ttml_lyrics` profiles](../docs/TTML.md). It is not the complete UniLyrics specification. Version `1` here identifies the experimental document shape; compatibility is not yet promised across pre-1.0 releases.

## Document

`Document` has a version, ordered cues, ordered key/value metadata, and an `attributes` object. Each cue has a unique nonempty ID, optional `start_us`/`end_us`, ordered segments, and attributes. Each segment has exact text, optional `start_us`/`end_us`, and attributes. Times are unsigned integer microseconds no larger than JavaScript's maximum safe integer. Zero and absent are distinct. Explicit intervals may not end before they start. Equal starts and overlapping cues are allowed; source order is retained.

Unknown top-level fields are rejected rather than silently dropped. The [rich-model extension](RICH_MODEL.md) adds typed agents, auxiliary content, languages, roles, cue parents, and segment identities, with reference/cycle validation. Attributes retain additional JSON data; TTML extension semantics and a stable editing contract remain unfinished. All ten SDK targets support [scoped custom format adapters](../docs/EXTENSIONS.md). The extension contract documents the shared callback-result validation and export-verification operations; the C/C++ ownership and callback exception contracts are documented alongside the managed-language APIs.

## Parsing and export

LRC accepts nonnegative minute counts, two-digit seconds, and zero to six fractional decimal digits. Seconds must be below 60. Repeated leading timestamps expand into separate cues. Metadata keys are alphabetic and duplicate entries remain ordered. Raw `offset` metadata is preserved with an `OFFSET_NOT_APPLIED` diagnostic; no dialect-independent offset direction is assumed. Colon fractions and proprietary vocal markers are not interpreted in phase 1.

Enhanced LRC preserves exact text spans and explicit trailing/end-gap markers. It does not invent missing vocal ends. Plain text preserves its content through a plain round trip, including CRLF and trailing line breaks. LRC input normalizes CRLF container separators to LF. A leading UTF-8 BOM is treated as encoding metadata and removed.

Canonical JSON is validated on import and exported deterministically. Strict export rejects reported feature loss; compatible export returns output and a list of losses. Target text that could be misread as timestamps/metadata is rejected where the profile has no escape syntax. Plain exports lose structured metadata/timing; baseline LRC loses segment timing and explicit cue ends. Attributes are reported as dropped by these text targets.

## Language boundary

Bindings share UTF-8 JSON requests:

```json
{"op":"parse","input":"[00:01.25]Hello","format":"lrc"}
```

Operations are `parse`, `detect`, `export`, `validate`, `inspect`, and [`normalize`](../docs/NORMALIZATION.md). Normalization takes a document and an optional signed integer `offset_us` (default zero), returning `{document, diagnostics}` without changing the input. Parse format may be omitted or null for detection. Export requires a document and format; policy defaults to `compatible`. Replies have either `{"ok":true,"result":...}` or `{"ok":false,"error":{"code":"...","message":"..."}}`. Parsing results contain `document` and `diagnostics`; exports contain `output` and `losses`.

Text inputs and encoded boundary requests each have a 1 MiB limit. JSON escaping/encoding adds overhead, so the maximum payload through a binding may be below the raw Rust text-input limit. Documents are limited to 10000 cues, 10000 agents, 10000 auxiliary entries, 100000 total segments, and 100000 annotation references; JSON recursion is bounded by serde_json's default recursion limit.

The C interface borrows input bytes for the duration of a call and returns an owned NUL-terminated UTF-8 JSON string. Call `unilyrics_free` exactly once. Invalid pointers cannot be made safe by Rust: C callers must supply a valid pointer/length and must not double-free. WASM loaders use the same protocol through bounded linear-memory allocations.

Built-in parsers run in the shared Rust core. Application-provided custom adapters execute locally in their host language, with returned data checked by the shared validator. SDKs do not fetch content or start parser subprocesses. Java/Kotlin expose shared typed records and map/list convenience methods alongside the lower-level JSON interface. C/C++ expose direct operation helpers with JSON responses and retain JSON model interfaces. Swift offers typed Codable models and Foundation JSON values; Go uses typed records; Python and JS return native JSON objects with model type declarations.

## Feature projection and profile exports

The shared protocol also supports [`project` and `export_profile`](../docs/PROFILES.md), using validated `ProjectionOptions`. Profiles retain declared features without inventing timing or changing primary text/IDs/segmentation. Projection removals are explicit diagnostics; profile export aggregates those with target-format losses, and strict policy rejects either source of loss. The separate [preservation bundle API](../docs/PRESERVATION.md) supplies canonical snapshots and convenience helpers in all ten languages. See [conversion behavior](../docs/COMPATIBILITY.md) and [platform requirements](../docs/PLATFORMS.md).
