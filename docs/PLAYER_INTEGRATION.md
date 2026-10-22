# Integrating lyrics into a player

[Documentation home](README.md)

Your player obtains lyrics from a file or provider, passes the text to UniLyrics, and renders the returned document against its own playback position. Fetching, caching, authentication, scrolling and animation stay in the app.

## Load once per track

Initialize the SDK once. Parse on track/lyrics changes, inspect available features, and retain the result with the track identity. Reject late provider responses for a track that is no longer playing. Do not parse on every animation frame.

Keep parse diagnostics available for troubleshooting. Show a static transcript if no usable cue timing exists. Choose translation/vocalist displays from declared fields rather than assuming every document has them.

## Select active cues

Convert your player's position to integer microseconds. For a millisecond clock, use `Math.round(positionMs * 1000)`. Apply any known user offset consistently, either to the document using normalization or to your display clock, once.

This TypeScript example selects **only explicitly bounded intervals**. It is app logic, not an SDK method. Use an ES module TypeScript file (`.mts`, or a project with `"type": "module"`):

```typescript
import type {Document} from "unilyrics";

function activeCues(document: Document, positionUs: number) {
  return document.cues.filter(cue =>
    cue.start_us !== null && cue.end_us !== null &&
    cue.start_us <= positionUs && positionUs < cue.end_us
  );
}
```

Use half-open intervals `[start, end)` to avoid keeping the previous cue active at its exact end. Return multiple cues: overlapping/background vocals are valid. For large documents, build a separate time index once; retain the original cue order and IDs for rendering and relationships.

## Choose a missing-end policy

LRC commonly gives starts without ends. The selector above intentionally omits those cues. Your player must decide how they remain visible. One possible display policy is to use the next strictly later cue start as a temporary end and the known track duration for the final cue. Simultaneous starts need grouping. Another policy keeps a cue visible until explicitly replaced.

These are display estimates, not source timing. Keep them in a separate UI index. Do not silently write estimated ends into the canonical document or treat them as valid word durations. Cues with no start belong in a static/untimed presentation.

## Highlight segments

Join segment text exactly for the line. For each segment with explicit start and end, compute progress as `(positionUs - startUs) / (endUs - startUs)`, clamped to `0..1`, only when `endUs > startUs`. Handle zero-duration segments as an instantaneous transition. Do not divide by zero or infer timed words by splitting untimed text. Segment timestamps are absolute track times, not cue-relative.

The SDK supplies data; your UI chooses fill animation, easing, scrolling and accessibility behavior. A segment boundary may be a syllable rather than a word.

## Handle seeks and related text

Recompute from the current position after seeks, loops and playback-rate changes; do not rely solely on incrementing a current-line counter. Pause uses the paused playback position. Guard asynchronous UI updates with the current track identity. Resolve translations/romanization through auxiliary `target_ids`, and agents and background relationships through IDs. See [data model](DATA_MODEL.md).

For browser apps, move synchronous parsing to a Worker if it affects UI response. Pass WASM bytes or a compiled module to that Worker's own SDK instance. App fetching of WASM assets is separate from the engine's offline parsing behavior.
