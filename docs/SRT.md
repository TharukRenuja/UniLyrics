# SRT profile

The `srt` format reads and writes indexed cue blocks with `HH:MM:SS,mmm --> HH:MM:SS,mmm` intervals. Clock fields must use colons and the millisecond separator must be a comma; misplaced or substituted separators are rejected. Hours may exceed two digits. Minutes and seconds must be below 60; milliseconds must have three digits. Explicit equal intervals, overlap, repeated indices, multiline Unicode text, and empty cue payloads are retained. Generated canonical cue IDs follow source order; a nondefault source index is retained in `attributes["srt:index"]` as a decimal string. Source indices are not assumed to be identities.

CRLF and CR line separators normalize to LF with a diagnostic. Markup remains verbatim text with an uninterpreted-markup diagnostic; this profile does not interpret styling or strip tags. Applications control rendering. It neither wraps long lines nor executes markup.

Export requires explicit cue starts and ends. It does not estimate durations from adjacent cues. Compatible export floors submillisecond times and reports precision loss; strict export rejects that loss. Rich annotations, auxiliary tracks, segment boundaries/timing and unsupported metadata/attributes produce losses. The exporter reparses its projection and reports any remaining canonical-model difference.

Embedded empty lines, leading/trailing payload newlines, and carriage returns cannot be preserved inside an SRT cue and are rejected. Position extensions, missing counters, non-comma timestamp dialects, and same-line counter/timestamp variants are outside this profile and rejected. Byte encoding detection is outside the string-based SDK API; callers must decode text before parsing.

Reference: [Library of Congress SRT description](https://www.loc.gov/preservation/digital/formats/fdd/fdd000569.shtml). SRT is not uniformly standardized; this document defines the supported profile rather than claiming every player dialect.
