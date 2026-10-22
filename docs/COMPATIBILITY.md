# Conversion behavior

[Documentation home](README.md)

A conversion can only retain information supported by the target format. UniLyrics makes removed information visible through loss diagnostics rather than silently treating the output as equivalent to the original document.

| Target | Appropriate use | Main limits |
|---|---|---|
| Canonical JSON | Store the complete shared model | Does not preserve the original file's byte layout |
| Plain text | Export readable untimed lyrics | Timing and richer structured features cannot be represented |
| LRC | Export synchronized lines | Word timing, auxiliary tracks and vocal structures exceed basic LRC |
| Enhanced LRC | Export supported inline timing | Rich relationships and auxiliary data exceed the format profile |
| SRT / WebVTT | Export subtitle cues | Require suitable cue intervals; rich lyrics metadata may be lost |
| TTML profiles | Exchange supported timed and vocal structures | Only the [documented profiles](TTML.md) are supported |

Use `compatible` to receive the supported output and its loss list. Use `strict` to reject a conversion when information would be lost. Compatible policy does not invent missing end times or bypass target-format requirements.

```javascript
const converted = sdk.export(document, "plain", "compatible");
for (const loss of converted.losses) {
  console.log(loss.code, loss.message);
}
```

If you need simplified output and a way to recover the original canonical document, use a [preservation bundle](PRESERVATION.md). If you want to deliberately retain only selected features, choose a [projection profile](PROFILES.md) before exporting.

Inspect the actual document before selecting a target: two inputs in the same format can contain different features and therefore produce different losses. Format details are listed in [format support](FORMATS.md).
