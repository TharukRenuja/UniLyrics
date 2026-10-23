# Canonical projection and export profiles

Profiles select which canonical features to retain. They never generate timestamps, split words, merge segments, reorder primary content, or remove primary cue/segment identities. A `word` profile retains supplied segment timing; segments may represent words, syllables or another source-defined unit. Missing timing stays missing.

The API is available directly in Rust, through every SDK's shared `process` protocol, and through the language helpers documented below. C/C++ helpers retain JSON document interfaces. Custom-format registries are unchanged; these profile export operations currently target built-in formats.

## Retention matrix

| Profile | Cue timing | Segment/aux timing | Agents/roles/relationships | Languages/metadata | Opaque attributes | Auxiliary content |
|---|---|---|---|---|---|---|
| `full` | Keep | Keep | Keep | Keep | Keep | All |
| `line` | Keep | Remove | Remove | Keep | Remove | Remove |
| `word` | Keep | Keep | Remove | Keep | Remove | Remove |
| `karaoke` | Keep | Keep | Keep | Keep | Remove | Remove |
| `plain` | Remove | Remove | Remove | Remove | Remove | Remove |
| `translated` | Keep | Keep | Keep | Keep | Remove | Only kind `translation` |
| `custom` | Explicit selection | Explicit selection | Explicit selection | Explicit selection | Explicit selection | Explicit selection |

Primary text, cue order, primary segment boundaries and IDs always remain intact. Background cues remain primary content; a profile may remove their annotations, but does not silently delete their text. Unused retained agents are not trimmed. Removing agents also clears agent references; removing relationships clears parents and auxiliary targets. Timing selections cover existing starts and ends together. Dropped auxiliary entries include their own text, timing and annotations.

Non-full presets intentionally remove opaque attributes: the SDK cannot know whether arbitrary extension data encodes discarded semantics. Custom profiles can retain attributes explicitly; if other data changes, `PROFILE_EXTENSIONS_UNCHANGED` warns that those opaque values may need application-specific updates.

## API

Rust:

```rust
use unilyrics::{project, export_profile, Format, Policy, Profile, ProjectionOptions};
let options = ProjectionOptions { profile: Profile::Line, features: None };
let projected = project(&document, &options)?;
let converted = export_profile(&document, Format::Lrc, &options, Policy::Compatible)?;
```

Shared protocol, used with each SDK's existing `process`/`Process` entry point:

```json
{"op":"project","document":{"version":1,"cues":[]},"options":{"profile":"line"}}
```

`project` returns `{document, diagnostics}` without mutating the input. Omitted options/profile default to `full`. An invalid document is rejected even for a full projection.

```json
{"op":"export_profile","document":{"version":1,"cues":[]},"format":"json","options":{"profile":"line"},"policy":"compatible"}
```

`export_profile` returns the usual `{output, losses}`. It reports projection losses first, followed by target-format losses. Strict policy rejects intentional profile removals as well as target-format losses; choosing a narrower profile does not make lost information lossless. Format requirements still apply: selecting a text-only profile does not make it possible to invent the timings required by SRT, for example.

## Custom selection

Use `profile: "custom"` with an explicit `features` list. Allowed names are:

- `line_timing`, `segment_timing`
- `agents`, `relationships`, `languages`, `roles`
- `metadata`, `attributes`, `auxiliary`

An empty list selects text/structure only; selecting all nine features is an identity projection. Omitted custom lists, duplicate selections, unknown features/options, and lists supplied to named profiles are rejected. Custom auxiliary selection keeps all auxiliary kinds; `translated` supplies the specific translation filter.

```json
{"op":"project","document":{"version":1,"cues":[]},"options":{"profile":"custom","features":["line_timing","languages","metadata"]}}
```

Loss codes use the `PROFILE_*_DROPPED` namespace and summarize each affected category. Projection preserves a valid reference graph, but is an explicit editing operation: applications manually composing it with custom exporters must retain its diagnostics rather than discarding them and claiming preservation against the original document.

The separate [preservation bundle API](PRESERVATION.md) retains original canonical data, including profile removals, in a sidecar. Convenience helpers are implemented in all 11 languages; platform verification is tracked separately. Source-byte preservation is not claimed.

## JavaScript/TypeScript and Python helpers

JavaScript/TypeScript exposes `sdk.project(document, options?)` and `sdk.exportProfile(document, format, options?, policy?)`. Python exposes `sdk.project(document, options=None)` and `sdk.export_profile(document, format, options=None, policy="compatible")`. Both default to the full profile and export policy defaults to compatible. TypeScript and Python export `Profile`, `ProjectionFeature`, and `ProjectionOptions`. Use `{profile: "custom", features: []}` (Python dictionary syntax as appropriate) for an explicit text-only selection. Rust remains the validation authority.

## Go helpers

Go exposes `Project(document, ProjectionOptions)` and `ExportProfile(document, format, ProjectionOptions, policy)`. Zero-value options select full; an empty policy selects compatible. Use `ProjectionOptions{Profile: ProfileCustom, Features: []ProjectionFeature{}}` for text/structure only. A nil custom Features slice is rejected. Profile and feature constants are exported; unknown string values remain core-validated.

## Swift helpers

Swift exposes `UniLyrics.project(_:options:)` and `UniLyrics.exportProfile(_:format:options:policy:)` for both `LyricsDocument` and Foundation dictionary documents. Typed calls use `LyricsProfile`, `ProjectionFeature` and `ProjectionOptions`; defaults select full/compatible. `ProjectionOptions(profile: .custom, features: [])` selects text/structure only; a nil custom feature list is rejected by the core.

## Java and Kotlin helpers

Both JVM SDKs expose `project` and `exportProfile` for maps and typed documents. Java uses `UniLyrics`; Kotlin uses `Lyrics`. Typed options are shared `LyricsModel.ProjectionOptions` with `Profile` and `ProjectionFeature` enums. The no-options calls select full and export policy defaults to compatible. `new ProjectionOptions(Profile.CUSTOM, List.of())` selects text/structure only; null custom features are rejected. Java overloads and Kotlin default arguments support passing explicit options and policy. Invalid map options reach Rust validation unchanged.

## C and C++ helpers

C exposes `unilyrics_project(document, length, options, options_length)` and `unilyrics_export_profile(document, length, format, strict, options, options_length)`. Options are a UTF-8 JSON object; `(NULL, 0)` defaults to full. Every returned JSON envelope belongs to the library and must be freed with `unilyrics_free`. The existing format codes apply, and strict is 0 (compatible) or 1 (strict).

C++ exposes `unilyrics::project(document_json, options_json="{}")` and `unilyrics::export_profile(document_json, format, options_json="{}", strict=false)`. These return owned `std::string` JSON envelopes, releasing native storage automatically. Both retain the native JSON document interface.

## C# and .NET helpers

`Lyrics.Project(document, options)` and `ExportProfile(document, format, options, policy)` return typed results. `ProjectionOptions` uses `Profile` and `ProjectionFeature` enums. Defaults select full/compatible. Use `new ProjectionOptions { Profile = Profile.Custom, Features = [] }` for explicit text/structure-only projection; omitted custom features are rejected by the core.
