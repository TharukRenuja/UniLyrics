# Examples

These small programs parse `[00:01]Hello 世界 🎵` with the shared engine. Most export the canonical document as JSON; C and C++ print the native parse-response envelope and demonstrate response ownership.

| Language | Source |
|---|---|
| Rust | [rust.rs](rust.rs) |
| JavaScript | [javascript.mjs](javascript.mjs) |
| TypeScript | [typescript.mts](typescript.mts) |
| Python | [python.py](python.py) |
| Go | [go.go](go.go) |
| Java | [Example.java](Example.java) |
| Kotlin | [kotlin.kt](kotlin.kt) |
| Swift | [swift.swift](swift.swift) |
| C | [c.c](c.c) |
| C++ | [cpp.cpp](cpp.cpp) |

Follow [installation](../docs/INSTALLATION.md) to build the engine and configure your language, then use the [language guides](../docs/LANGUAGES.md) for dependency and native-loader setup. The standalone examples use source-tree paths; adapt these to the package locations in your application.

The [Android application](android/README.md) is a complete Gradle consumer of the AAR. See [player integration](../docs/PLAYER_INTEGRATION.md) for using parsed lyrics with a playback clock and UI.
