# Installation

[Documentation home](README.md)

UniLyrics currently uses source dependencies and locally built packages. The commands below do not assume published npm, PyPI, Maven or crates.io releases. Keep native/WASM engines and wrappers from the same revision together.

## Build the engine

Install Rust/Cargo and your platform linker. From the repository root:

```sh
cargo fetch --locked
cargo build --locked --release -p unilyrics-ffi
```

The engine is written to `target/release`: `libunilyrics_ffi.so` on Linux, `unilyrics_ffi.dll` on Windows, or `libunilyrics_ffi.dylib` on macOS. Source compilation alone does not establish a supported platform distribution; see [platforms](PLATFORMS.md).

## Build local packages

The complete Linux x86_64 package builder requires Python 3.11+, pip, setuptools 70.1+, Rust, CMake, GCC, Node/npm, JDK 17, Kotlin and the .NET 10 SDK. Fetch Cargo dependencies first and install the WASM target:

```sh
rustup target add wasm32-unknown-unknown
```

Supply JSON-java `json-20250517.jar` at `target/deps/json-20250517.jar`, or set `UNILYRICS_JSON_JAR` to its absolute path. Obtain it from [Maven Central](https://repo.maven.apache.org/maven2/org/json/json/20250517/json-20250517.jar); its SHA-256 is `3ea61b2a06e31edf1c91134fe9106b0ebb16628be169f3db75bc7a2b06b45796`.

```sh
python -B scripts/package.py
```

Artifacts are written to `target/packages`, with a checksum manifest. The builder replaces its own staging and output directories, builds offline from installed dependencies and does not publish packages. If you only need one binding, use the source integration below instead of installing every language toolchain.

For Linux-hosted Windows x64 MinGW cross-packages, install the `x86_64-pc-windows-gnu` Rust target, MinGW GCC, CMake and Python/pip/setuptools, then run `python -B scripts/package_windows.py`. Outputs go to `target/packages-windows`. These packages remain preliminary; they are not MSVC builds.

## JavaScript and TypeScript

Install a locally built npm archive:

```sh
npm install /path/to/unilyrics-0.1.0.tgz
```

```javascript
import { readFile } from "node:fs/promises";
import { initialize } from "unilyrics";
const sdk = await initialize(await readFile(new URL(import.meta.resolve("unilyrics/core.wasm"))));
```

TypeScript declarations are included in the same package. In browsers, load the packaged `core.wasm` through your application's asset pipeline and pass its bytes or a compiled `WebAssembly.Module` to `initialize`. Initialize once and reuse the SDK.

For source integration, build the engine for `wasm32-unknown-unknown` and copy `target/wasm32-unknown-unknown/release/unilyrics_ffi.wasm` to `bindings/javascript/dist/core.wasm` before packaging. The loader itself does not download an engine.

## Python

Python 3.9+ is declared by the wrapper. Install the wheel built for your operating system and architecture:

```sh
python -m pip install /path/to/unilyrics-0.1.0-py3-none-linux_x86_64.whl
```

Windows cross-packages use the filename `unilyrics-0.1.0-py3-none-win_amd64.whl`. Wheel tags identify the target, not a guarantee of every OS/interpreter combination.

```python
from unilyrics import UniLyrics
sdk = UniLyrics()
parsed = sdk.parse("[00:01.25]Hello", "lrc")
```

Bundled wheels load their included engine. For a source installation (`python -m pip install ./bindings/python`), pass an absolute library path to `UniLyrics(...)` or set `UNILYRICS_LIBRARY`. To build a bundled wheel yourself, set the build-time variable `UNILYRICS_NATIVE_LIBRARY` to the engine and run `python -m pip wheel --no-build-isolation --no-deps ./bindings/python`.

## Java and Kotlin

For the Linux package, put `unilyrics-0.1.0-linux-x86_64.jar` and `json-20250517.jar` on the classpath. For Kotlin, add `unilyrics-kotlin-0.1.0.jar` and the Kotlin standard library. JSON-java is a separate dependency declared by the Java POM. Maven repository publication is not configured.

```java
import org.tharukrj.unilyrics.UniLyrics;
UniLyrics.load();
var parsed = UniLyrics.parse("[00:01.25]Hello", "lrc");
```

`load()` extracts bundled native libraries into a private temporary directory and loads them. That filesystem must permit native library loading. For application-managed libraries, call `UniLyrics.loadSystem()` (Kotlin: `Lyrics.loadSystem()`) with both the engine and JNI bridge on the native search path. The Java API includes typed Java 17 records as well as map/list methods. Kotlin uses the same model.

Android uses its own AAR and platform JSON classes. Follow [Android integration](../bindings/android/README.md); do not add the desktop JSON-java JAR to an Android app.

## C# and .NET

The binding targets .NET 10. Install the .NET 10 SDK to build it; consuming applications must target .NET 10 or a compatible later framework. This is not a .NET Framework or Unity package.

On Linux x64, build a local package containing the managed assembly and native engine:

```sh
python -B scripts/package_dotnet.py
```

To also bundle an existing Windows x64 engine, pass `--windows-native /absolute/path/to/unilyrics_ffi.dll`. The builder does not compile that Windows DLL; build it separately with the matching Rust sources. Outputs go to `target/packages-dotnet` and are not uploaded anywhere.

From your application's directory:

```sh
dotnet add package UniLyrics --version 0.1.0 --source /absolute/path/to/packages-dotnet
```

```csharp
using UniLyrics;
using var lyrics = new Lyrics();
var parsed = lyrics.Parse("[00:01.25]Hello", Format.Lrc);
```

The package must contain a native asset matching the application's OS and CPU. A Linux-only package cannot run on Windows. For source project references or external native libraries, pass an absolute path to `new Lyrics(...)` or set `UNILYRICS_LIBRARY`. Dispose the engine after the application finishes using it. See the [.NET guide](../bindings/dotnet/README.md) for model types, errors and custom formats.

## C and C++

Install the native engine, headers and CMake package:

```sh
cmake -S bindings/c -B target/cmake -DUNILYRICS_NATIVE_DIR=/absolute/path/to/target/release
cmake --install target/cmake --prefix /absolute/path/to/sdk
```

Alternatively, extract a native SDK archive produced by the package builder. In your application's CMake project:

```cmake
find_package(UniLyrics 0.1 CONFIG REQUIRED)
target_link_libraries(my_c_app PRIVATE UniLyrics::unilyrics)
target_link_libraries(my_cpp_app PRIVATE UniLyrics::cpp)
```

Configure with `-DCMAKE_PREFIX_PATH=/absolute/path/to/sdk`. Include `<unilyrics.h>` or `<unilyrics.hpp>`. C responses are library-owned allocations: release each exactly once with `unilyrics_free`. The C++17 wrapper returns owned strings and handles response cleanup.

## Go

The module path is `github.com/TharukRenuja/UniLyrics/bindings/go`. Until versioned modules are published, use a local replacement in your application's `go.mod`:

```go
require github.com/TharukRenuja/UniLyrics/bindings/go v0.0.0
replace github.com/TharukRenuja/UniLyrics/bindings/go => /absolute/path/to/UniLyrics/bindings/go
```

Import that module path. Enable cgo and set `CGO_LDFLAGS` to locate the matching native library, for example `-L/absolute/path/to/target/release -Wl,-rpath,/absolute/path/to/target/release` on Linux. Go requires a C compiler; consumers using a prebuilt engine do not need Rust.

## Swift

Add `bindings/swift` as a local SwiftPM package dependency and select its `UniLyrics` product. Supply the engine's location when building your application:

```sh
swift build -Xlinker -L/absolute/path/to/native -Xlinker -rpath -Xlinker /absolute/path/to/native
```

The wrapper offers Codable models, Foundation JSON values and structured errors. It is a source wrapper with a separate native library, not an Apple binary XCFramework. macOS/iOS deployment and distribution remain in development.

## Rust

Use the core as a path dependency:

```toml
[dependencies]
unilyrics = { path = "/absolute/path/to/UniLyrics/crates/unilyrics" }
```

Cargo builds the engine directly; Rust callers do not need the C ABI. The Linux package builder also produces a standalone `.crate` source archive that can be extracted and used as a path dependency. Registry publication is disabled.

Next, follow your [language guide](LANGUAGES.md) and the [API workflows](API.md).
