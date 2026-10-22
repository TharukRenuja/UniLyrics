# Errors and troubleshooting

[Documentation home](README.md)

## Handle errors separately from diagnostics

A failed operation returns no successful result. A successful parse can still return diagnostics, and a compatible conversion can return losses. Check those lists rather than treating success as proof that every feature survived.

JavaScript throws `UniLyricsError`; Python raises `UniLyricsError` with `.code`; Java/Kotlin use `UniLyrics.LyricsException` with `getCode()`. Go and Rust return errors; Swift throws. C/C++ callers inspect the JSON envelope's `ok` and `error` fields. C++'s ownership wrapper returning a string does not mean the operation inside that JSON succeeded.

```python
from unilyrics import UniLyrics, UniLyricsError

sdk = UniLyrics()
try:
    parsed = sdk.parse("[00:01.25]Hello", "lrc")
    converted = sdk.export(parsed["document"], "plain", "strict")
except UniLyricsError as error:
    if error.code == "LOSSY_CONVERSION":
        print("Choose a richer target or explicitly accept conversion losses.")
    else:
        raise
```

| Code | What to check |
|---|---|
| `INPUT_TOO_LARGE` | Raw text and encoded request budgets, including JSON escaping/sidecars |
| `INVALID_JSON` / `INVALID_REQUEST` | Valid JSON and the documented operation/field types |
| `INVALID_TIMESTAMP` | Syntax and range for the selected source format |
| `INVALID_TIME` / `INVALID_OFFSET` | Model timing and signed normalization range |
| `MISSING_TIMING` | Subtitle export requires explicit usable cue intervals |
| `LOSSY_CONVERSION` | Strict export/profile would remove information |
| `SIDECAR_MISMATCH` | Restore requires the exact paired output and format |
| `UNSUPPORTED_SIDECAR_VERSION` | Preservation bundle version must be supported |
| `AMBIGUOUS_FORMAT` | Choose an explicit format when registry matches overlap |

This is a troubleshooting selection, not an exhaustive error-code enum. Match codes where you need application behavior; messages provide human-readable detail. Do not retry unchanged malformed input indefinitely.

## Size and model limits

Raw text and encoded shared requests each have a 1 MiB limit. Escaping adds request overhead. Documents allow up to 10000 cues, 10000 agents, 10000 auxiliary entries, 100000 total segments and 100000 annotation references. Times are integer microseconds in `0..9007199254740991`. JSON nesting is bounded. The shared core validates IDs, references and timing. See the [contract](../spec/README.md).

## Native library will not load

Use a binary for the process OS and CPU, and keep wrapper, engine and JNI bridge versions together. A Linux `.so` is not an Android library despite its extension. Ensure linker dependencies are available and library search paths point to the extracted SDK. Current Linux artifacts require glibc symbols through 2.34.

Python supports an explicit library path or `UNILYRICS_LIBRARY`. JVM `load()` uses bundled resources; `loadSystem()` expects app/system-supplied matching engine and JNI libraries. JVM extraction requires a location that permits native loading. Go requires cgo at build time. Swift requires native linker paths. See [installation](INSTALLATION.md) for the exact setup.

## WASM initialization fails

Pass WASM bytes, an ArrayBuffer/view or a compiled WebAssembly.Module, not a URL string or unawaited fetch response. Check the asset path and HTTP status before reading bytes. Supply a compatible engine from the same package. Browser content security policy and Worker setup belong to the host app.

## Platform packaging

Android requires API 29+ and matching AAR native libraries. Preserve the AAR consumer rules when enabling R8. Kotlin 2.4 requires compatible Android compiler tooling; a standalone D8 JAR does not replace Gradle's embedded compiler. See [Android integration](../bindings/android/README.md).

macOS/iOS binary distribution remains in development. A successful source build on another operating system does not establish Apple linking or runtime compatibility. See [platform requirements](PLATFORMS.md).
