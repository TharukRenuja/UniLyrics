# Experimental rich document model

This extends the pre-1.0 version-1 document shape. Existing plain/LRC documents still serialize in their previous shape: new optional fields and empty optional collections are omitted. The new relationships currently enter through canonical JSON or direct model construction. **TTML parsing/export is not implemented yet.**

## Fields

| Location | Added fields | Meaning |
|---|---|---|
| Document | `language?`, `agents?`, `auxiliary?` | Declared document language, vocal identities, auxiliary content |
| Cue | `language?`, `agent_ids?`, `roles?`, `parent_id?` | Declared language, singers, open role tokens, optional cue parent |
| Segment | `id?`, `language?`, `agent_ids?`, `roles?` | Stable alignment target, language, singers, open role tokens |
| Agent | `id`, `name?`, `kind?`, `attributes` | Source identity, optional name/type, extensions |
| Auxiliary | `id`, `kind`, `language?`, `target_ids`, `segments`, `attributes` | Translation, romanization, pronunciation, or an opaque source kind |

`?` means optional. An auxiliary entry has its own exact segments and optional segment timing. It does not need the same number of segments as its source. `target_ids` can reference one or more main-content cue/segment IDs. An empty target list represents document-wide auxiliary content; the SDK does not invent alignment. Auxiliary segments may have IDs, but those IDs are not accepted as auxiliary targets in this initial relationship model.

Example conventions are cue roles `lead`, `background`, and `instrumental`; auxiliary kinds `translation`, `romanization`, and `pronunciation`. Role/kind strings remain open so unknown source values survive JSON round trips. An unknown value is retained, not interpreted as a supported platform extension. No voice identity, translation, language, word split, missing timing, or timing containment is inferred.

Use a background cue's `parent_id` to relate it to a lead cue while keeping its independent timing and text. Other group relations can use the same optional parent structure. Source cue order remains unchanged, even when starts overlap or run backward. Parent references are structural; they do not force timing containment or timing/language inheritance.

Language strings are retained as declared. Empty declarations are rejected, but this version does not certify BCP 47 syntax or infer a script/language from text. Text is not normalized, trimmed, or respaced.

## Validation

- All cue, explicit segment, agent, and auxiliary IDs are nonempty and globally unique.
- Vocal references must name declared agents. Parent references must name cues and be acyclic.
- Auxiliary targets must name main-content cues or their explicitly identified segments.
- Role/kind values must be nonempty when declared. Agent names may be empty source text.
- Existing safe microsecond and interval validation applies to auxiliary segments as well as main content.
- Limits are 10,000 cues, 10,000 agents, 10,000 auxiliary entries, 100,000 total segments, and 100,000 total role/vocal/auxiliary-target references. Encoded boundary requests still have their separate 1 MiB limit.
- Parent chains are checked iteratively, visiting each cue at most once per validation. Nesting cannot exhaust the call stack.

Dangling references fail with `UNKNOWN_AGENT`, `UNKNOWN_PARENT`, or `UNKNOWN_TARGET`. Parent cycles fail with `CYCLIC_RELATIONSHIP`; duplicate/empty IDs fail with `INVALID_ID`. Inspection and every exporter validate before processing.

## Feature inspection

Rust exposes `inspect(&document) -> Result<Features, Error>`. Other SDKs expose `inspect(document)` (`Inspect` in Go), through the shared `{"op":"inspect","document":...}` protocol. C/C++ helpers consume canonical document JSON, returning the same owned JSON response shape as their other operations.

The result contains:

| Field | Meaning |
|---|---|
| `line_timing` | Any cue has an explicit start or end |
| `segment_timing` | Any main/auxiliary segment has a start or end |
| `explicit_ends` | Any cue or main/auxiliary segment has an end |
| `agents` | The agent registry is nonempty |
| `relationships` | A cue parent or explicit auxiliary target is present |
| `languages` | Sorted unique declared languages across document/cues/segments/auxiliary entries |
| `roles` | Sorted unique cue/segment roles, including auxiliary segments |
| `auxiliary_kinds` | Sorted unique auxiliary kinds, including unknown kinds |
| `extensions` | Any document/cue/segment/agent/auxiliary attributes are nonempty |

This describes facts present in the document. It does not report provider availability, prove a format can represent every feature, identify words from timed runs, or classify an unknown extension as implemented. Inspection never mutates the document or fills missing values.

## Conversion guarantees

Canonical JSON retains these fields and relationships. Plain/LRC/Enhanced LRC cannot represent them in the implemented profiles. Compatible exports preserve main text and supported timing while reporting the applicable `AGENTS_DROPPED`, `AUXILIARY_DROPPED`, `LANGUAGE_DROPPED`, `VOCAL_AGENTS_DROPPED`, `ROLES_DROPPED`, `RELATIONSHIPS_DROPPED`, and `SEGMENT_IDS_DROPPED` losses. Auxiliary text is not mixed into sung main text. Existing timing/metadata/extension losses still apply.

Text targets regenerate cue IDs from line order, so custom IDs now also produce `CUE_IDS_CHANGED`. Strict export rejects reported losses with `LOSSY_CONVERSION`. This intentionally makes strict conversion reject cases where earlier experimental builds failed to report identity loss. Loss reporting is not a guarantee of byte-identical source reconstruction or arbitrary unknown-extension editing.

Auxiliary tracks may have independent segment counts and timing. References must target existing IDs, and vocal parent relationships must be acyclic.

