# Experimental TTML profiles

Use `ttml` for W3C parent-relative media timing, or `ttml_lyrics` for absolute music timestamps. The parser never decides that a nested timestamp is absolute based on its numeric value. Detection rejects recognizable music extensions as ambiguous until the caller selects a profile.

The supported subset includes namespaced paragraphs/spans, explicit starts/ends/durations, line breaks, inherited languages/whitespace, vocal agents, and inline music background/translation/romanization roles. Music head auxiliary tracks can target iTunes lyric keys. Times use exact integer microseconds; unsupported precision is rejected. Missing line ends are not inferred from the next line.

XML DTDs and processing instructions are rejected, input/node/depth limits apply, and foreign structural elements are rejected. Uninterpreted attributes are retained with diagnostics; styling/rendering semantics are not implemented. This is not a full TTML schema validator.

Music export preserves translations and romanization with a single cue target through head tracks, including targets that are background cues. Track timing remains independent from the lyric interval. Background cues with an explicit background role and contained intervals are nested under their parent; export preserves nested relationships when the resulting document order is representable. Duplicate key/value metadata is emitted using AMLL metadata entries.

Standard export uses a flat paragraph/span projection. Unsupported auxiliary kinds, multiple/segment targets, unrepresentable relationships, and uninterpreted attributes are reported as losses. Every export reparses its output and compares the full model; strict mode rejects any difference. Canonical JSON remains the complete preservation format.

Timing references: [W3C TTML2](https://www.w3.org/TR/ttml2/), [Apple music asset guide](https://help.apple.com/itc/videoaudioassetguide/en.lproj/static.html), and [AMLL TTML conventions](https://github.com/amll-dev/amll-ttml-db/blob/main/instructions/ttml-specification-en.md). Supporting these subsets does not imply certification by a platform.
