import UniLyrics

let parsed = try UniLyrics.parseTyped("[00:01]Hello 世界 🎵", format: .lrc)
let converted = try UniLyrics.export(parsed.document, format: .json, policy: .strict)
print(converted.output)
