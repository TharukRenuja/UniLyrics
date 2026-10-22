# Supported formats

[Documentation home](README.md)

Use these exact identifiers in string-based SDK APIs. Typed SDKs provide matching enums; see [language guides](LANGUAGES.md).

| Identifier | Intended use | Important boundary |
|---|---|---|
| `plain` | Untimed text | Cannot represent structured timing or vocal annotations |
| `lrc` | Line-synchronized lyrics | Explicit cue ends and segment timing are lost on export |
| `enhanced_lrc` | Inline timed lyric spans | Does not invent absent word/vocal ends |
| `json` | Versioned UniLyrics canonical document | Validates the shared model; not arbitrary provider JSON |
| `srt` | Subtitle intervals | Requires usable explicit cue starts and ends |
| `webvtt` | Web subtitle/cue profile | Bounded experimental profile; see [WebVTT](WEBVTT.md) |
| `ttml` | Standard timed-text profile | Bounded experimental profile; see [TTML](TTML.md) |
| `ttml_lyrics` | Music-oriented TTML profile | Only documented music constructs are interpreted |

The [wire specification](../spec/README.md) defines accepted timestamp syntax, metadata and text behavior. Read [SRT](SRT.md), [WebVTT](WEBVTT.md) and [TTML](TTML.md) before relying on their specialized features.

A format name does not promise every vendor dialect. Proprietary compressed or provider-specific payloads need explicit adapters; YRC, QRC, KRC and Lyricify are not built-in format identifiers. UniLyrics does not fetch Apple Music, Spotify, YouTube Music or Musixmatch content.

Choose `strict` when loss should stop a conversion. Choose `compatible` when your app can accept and show the reported losses. Save a preservation sidecar when you need to recover the original canonical model alongside simplified target text. See [API workflows](API.md) and [checked compatibility examples](COMPATIBILITY.md).
