# Working with the lyrics document

[Documentation home](README.md)

A parse result contains `document` and `diagnostics`. The document uses version 1. Bindings expose its fields through native objects, records or canonical JSON; [language guides](LANGUAGES.md) explain the representation for each SDK.

```json
{
  "version": 1,
  "cues": [{
    "id": "line-1",
    "start_us": 1250000,
    "end_us": null,
    "segments": [{
      "text": "Hello",
      "start_us": null,
      "end_us": null,
      "attributes": {}
    }],
    "attributes": {}
  }],
  "metadata": [],
  "attributes": {}
}
```

This is a minimal authored example, not a promise about generated cue IDs.

## Cues and segments

A cue groups text segments with an ID and optional start/end. Display text is the concatenation of its segments in order; do not insert spaces unless your UI intends to change the text. Segment boundaries may describe words, syllables or other source spans. They are not guaranteed to be linguistic word boundaries.

Cue and segment times are absolute positions on the same track timeline, in integer microseconds. `1000000` is one second. `null` means unknown; zero is a valid known time. Do not substitute zero for missing timing. Supported times range from zero through `9007199254740991`. The core validates ordering and relationships.

The SDK preserves source cue order, repeated starts and concurrent cues. Do not assume every document is sorted or that only one cue can be active.

## Metadata and extensions

`metadata` is an ordered list of key/value entries; duplicate keys are allowed. Converting it to a dictionary can discard information. `attributes` holds opaque JSON extension values on documents and supported child objects. The SDK preserves or reports their loss without interpreting arbitrary application semantics.

LRC offset metadata is retained with a diagnostic rather than applied implicitly. Use [explicit normalization](NORMALIZATION.md) when you know the intended shift.

## Languages, vocalists and related text

Documents, cues and segments may declare `language`. Agents have IDs and optional names/kinds; `agent_ids` associates cues/segments with declared agents. `roles` retains vocal roles, while cue `parent_id` represents relationships such as a background vocal associated with another cue.

`auxiliary` entries hold related text such as translations and romanization. Each has an ID, an open `kind`, optional language, `target_ids`, its own segments and attributes. Match targets by ID, not by array position. Auxiliary segments may have independent timing. Languages, roles and translations are supplied by the source or your app; parsing does not generate them.

Use `inspect(document)` to discover available timing, languages, roles, relationships and auxiliary kinds before choosing a UI. Validate authored or modified documents before using them. For complete field definitions and reference constraints, see the [rich model specification](../spec/RICH_MODEL.md) and [wire contract](../spec/README.md).
