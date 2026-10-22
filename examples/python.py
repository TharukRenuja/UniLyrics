from unilyrics import UniLyrics

sdk = UniLyrics()  # Uses UNILYRICS_LIBRARY for this source-tree example.
document = sdk.parse("[00:01]Hello 世界 🎵", "lrc")["document"]
print(sdk.export(document, "json", "strict")["output"])
