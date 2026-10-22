# Experimental WebVTT profile

Use `webvtt` or `Format::Webvtt`; native C format code 8 is `UNILYRICS_WEBVTT`. The signature is detected independently from plain text and SRT. This is a documented subset, not a complete WebVTT conformance checker.

## Imported data

Cue intervals use exact millisecond timestamps, spelled `MM:SS.mmm` or `HH:MM:SS.mmm`. Separator positions are checked for both cue boundaries and inline timestamps; a colon before milliseconds or misplaced decimal point is rejected. Starts must not decrease; overlap is allowed and each end must be later than its start. Cue identifiers may repeat: canonical cue IDs follow source order, while original identifiers remain in `webvtt:identifier` attributes.

Inline timestamps produce segment starts. Their values must increase strictly within the cue interval. They do not imply explicit segment ends. Consecutive and trailing timestamps retain empty timed segments rather than inventing words.

Voice spans create a deterministic agent registry with explicit names and `voice` kinds; segments reference those agents. Language spans populate segment languages without full BCP 47 authoring validation. Class, bold, italic, underline, ruby, ruby-text, voice, and language spans retain their source grouping in `webvtt:tags`. Ruby pronunciation segments carry the open role `ruby_text`. Cue text concatenates main and ruby text; consumers can use the role to display them separately.

Header suffixes/metadata, cue settings, NOTE comments, STYLE CSS, and REGION blocks remain in `webvtt:*` attributes. Their positioning is retained. Diagnostics state that CSS/layout and timestamp-map interpretation are not performed. The SDK does not execute styling or render captions. CRLF/CR separators normalize to LF with a diagnostic.

## Export and limits

Export reconstructs retained span paths and non-cue blocks and reparses the entire result. Strict export rejects a changed canonical projection. Compatible export reports changed projections and millisecond precision loss. It requires explicit cue starts/ends and never estimates durations.

For segments without retained span paths, export now generates a single voice and language span from declared canonical annotations. Segment declarations take precedence over cue/document context; moving a cue/document declaration to segments is detected as a changed projection. Simultaneous agent references are not reduced to a guessed single singer: the loss is reported. Arbitrary role synthesis and preservation of source-independent agent IDs/kinds remain limited by the target format.

Newly generated span paths are additional serialization provenance, so strict comparison permits those added attributes when the input had no retained path. Typed text, timing, language, identity and relationship differences remain checked. Existing paths are retained; typed edits conflicting with those paths still produce a projection difference. Remove a retained path when explicitly choosing to regenerate its supported annotations. Canonical JSON remains the complete model-preservation format.

An explicit segment start equal to the cue start cannot be written as a WebVTT inline timestamp. Compatible export omits that marker with `SEGMENT_START_IMPLICIT`; strict export rejects the change. Missing ends are never estimated.

This profile accepts six named character references: amp, lt, gt, nbsp, lrm, and rlm. Other references, foreign tags, malformed nesting, and excessive span depth are rejected explicitly. It supports a final voice span with an omitted closing tag and the final ruby-text closing-tag omission; other unclosed spans are rejected. Embedded blank lines, boundary newlines, carriage returns, and NUL cannot be preserved in cue payloads. Layout settings are retained rather than authoring-validated. Unsupported metadata-track payload modes and the full browser error-recovery algorithm remain future work.

Reference: [W3C WebVTT](https://www.w3.org/TR/webvtt1/). The shared conformance corpus includes an exact voice/language/timestamp model for every SDK.
