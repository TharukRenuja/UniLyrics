# Platforms

[Documentation home](README.md)

The bindings share one engine, but package availability differs by platform. Use a native library built for your target OS and architecture; matching filenames alone do not establish compatibility.

| Platform | Integration | Availability |
|---|---|---|
| Linux x86_64 | All ten language interfaces | Local package builder available; no universal glibc baseline promised |
| Android API 29+ | Java/Kotlin AAR with ARM64 and x86_64 native libraries | AAR builder and Gradle example available |
| Windows x86_64 | Native C ABI, C/C++, Go and Python interfaces | MinGW cross-package builder available; native-host compatibility remains preliminary |
| macOS | Rust core, C interface and Swift source wrapper | Native distribution and deployment targets are still in development |
| iOS | Intended Swift/C integration | No ready-to-use XCFramework or supported binary distribution yet |
| Node / browser | JavaScript and TypeScript through WASM | Caller supplies WASM bytes or a compiled module; browser bundling is application-managed |

Java requires Java 17 or newer; Kotlin is a JVM facade, not Kotlin/Native or Kotlin Multiplatform. Swift uses SwiftPM with a separate native engine. Go requires cgo. See [installation](INSTALLATION.md) for prerequisites and paths.

Android's minimum API is 29. The native libraries use at least 16 KB ELF LOAD alignment; applications must also package their APK correctly for the devices they target. The AAR includes JNI consumer rules, and release consumers should retain those rules when shrinking. See [Android integration](../bindings/android/README.md).

Native packages are experimental local artifacts. Do not assume that a source binding implies a supported binary for every OS, CPU, runtime version or distribution channel. Pin the wrapper and engine to the same version.
