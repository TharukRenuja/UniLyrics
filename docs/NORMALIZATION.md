# Explicit normalization

The first normalization operation applies a caller-selected timing offset to the canonical document. It is available in every SDK and runs in the shared Rust core. Parsing does not apply it automatically.

`normalize(document, offset_us)` returns the same `{document, diagnostics}` shape as parsing. The input is never modified. A positive offset moves existing times later; a negative offset moves them earlier. Units are signed integer microseconds. The protocol defaults an omitted offset to zero.

## Timing guarantees

- Shift every present cue start/end, cue segment start/end, and auxiliary segment start/end by the same amount.
- Keep absent boundaries absent. Preserve zero as an explicit time.
- Preserve durations, overlaps, simultaneous starts, ordering, text, IDs, relationships, languages, agents, metadata and attributes.
- Reject the entire operation if any shifted time falls outside `0..9007199254740991`; do not clamp, truncate, reorder, or return a partial document.
- Reject offsets outside `-9007199254740991..9007199254740991`. Validate the input even for a zero offset.

A zero offset returns the validated document unchanged with no diagnostics. A nonzero offset on a document without explicit times also leaves it unchanged.

## Diagnostics and opaque source data

`TIMING_SHIFTED` reports that explicit times changed. `EXTENSIONS_UNCHANGED` warns when the document contains opaque attributes: those values are preserved verbatim, including any embedded source timestamps. The engine cannot safely rewrite the meaning of arbitrary extension data. Applications may need to update these attributes before exporting into their source format; ordinary conversion-loss checks still apply.

`OFFSET_METADATA_UNCHANGED` reports retained metadata named `offset` (case-insensitive). This operation does not infer that metadata's sign, units, or dialect semantics, consume it, or apply it a second time. The caller selects the actual microsecond offset explicitly. Metadata order and duplicate entries remain intact.

Range failures use `INVALID_TIME` or `INVALID_OFFSET`. Noninteger/wrong-type offsets at the JSON boundary use `INVALID_REQUEST`. All failures use the existing structured error interface.

## Language APIs

| SDK | Call |
|---|---|
| C# / .NET | `lyrics.Normalize(document, -250_000)` |
| Rust | `unilyrics::normalize(&document, -250_000)?` |
| JavaScript/TypeScript | `sdk.normalize(document, -250000)` |
| Python | `sdk.normalize(document, -250_000)` |
| Go | `unilyrics.Normalize(document, -250000)` |
| Java | `UniLyrics.normalize(document, -250_000L)` |
| Kotlin | `Lyrics.normalize(document, -250_000L)` |
| Swift | `try UniLyrics.normalize(document, offsetUs: -250_000)` |
| C | `unilyrics_normalize(bytes, length, -250000)` |
| C++ | `unilyrics::normalize(document_json, -250000)` |

Java/Kotlin and Swift support both their typed document and existing map/Foundation interfaces. C/C++ take canonical document JSON and return the ordinary JSON envelope; C callers free it with `unilyrics_free` exactly once. The direct C function is a separate native symbol; existing `unilyrics_call` operation codes are unchanged.

Protocol example:

```json
{"op":"normalize","document":{"version":1,"cues":[]},"offset_us":-250000}
```

This phase does not implement text/Unicode cleanup, whitespace changes, cue sorting, time inference, quantization, metadata interpretation, or custom parser/exporter registration. Those need separately specified controls rather than implicit alterations to imported lyrics.
