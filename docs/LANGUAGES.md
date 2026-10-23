# Language guides

[Documentation home](README.md)

Follow [installation](INSTALLATION.md) first. Paths below assume installed local packages, except Rust's extracted source dependency and C/C++'s extracted SDK. All snippets use explicit LRC; see the [complete examples](../examples/README.md) for executable programs and source-tree setup.

## Rust

Add a Cargo path dependency pointing at the extracted `.crate` directory:

```toml
[dependencies]
unilyrics = { path = "/path/to/unilyrics-0.1.0" }
```

```rust
use unilyrics::{parse, export, Format, Policy};

fn main() -> Result<(), unilyrics::Error> {
    let parsed = parse("[00:01.25]Hello", Some(Format::Lrc))?;
    let converted = export(&parsed.document, Format::Json, Policy::Strict)?;
    println!("{}", converted.output);
    Ok(())
}
```

Rust uses the core directly and builds it with Cargo. No separate native loader is needed. See [Rust example](../examples/rust.rs) and the [core API source](../crates/unilyrics/src/lib.rs).

## JavaScript and TypeScript

Install the local npm tarball. TypeScript declarations are included in the same package. In Node:

```javascript
import {readFile} from "node:fs/promises";
import {initialize} from "unilyrics";

const sdk = await initialize(
  await readFile(new URL(import.meta.resolve("unilyrics/core.wasm")))
);
const {document} = sdk.parse("[00:01.25]Hello", "lrc");
console.log(sdk.export(document, "json", "strict").output);
```

In a browser, copy `core.wasm` from the installed package into your app's assets, then initialize using your chosen asset URL:

```javascript
import {initialize} from "unilyrics";

const response = await fetch("/assets/unilyrics/core.wasm");
if (!response.ok) throw new Error(`WASM load failed: ${response.status}`);
const sdk = await initialize(await response.arrayBuffer());
```

Asset delivery is your app's responsibility. The initialized parser performs no network requests. The same initialization accepts a compiled WebAssembly.Module inside a Worker. Initialize one SDK per execution context and reuse it.

TypeScript exposes `Document`, `Cue`, `Segment`, `Format`, `Policy` and result types. JS/TS results are ordinary objects. See [declarations](../bindings/javascript/index.d.ts), [JavaScript example](../examples/javascript.mjs), and [TypeScript example](../examples/typescript.mts).

## Python

Install the platform wheel. Linux installed consumers pass; the Windows x64 development wheel has packaging checks but awaits Windows Python execution. See [installation](INSTALLATION.md#windows-x64-development-wheel). Its bundled engine allows construction without a path:

```python
from unilyrics import UniLyrics

sdk = UniLyrics()
parsed = sdk.parse("[00:01.25]Hello", "lrc")
print(sdk.export(parsed["document"], "json", "strict")["output"])
```

For a source-only wrapper, pass the matching native library path or set `UNILYRICS_LIBRARY`. Public TypedDict models describe ordinary dictionaries; runtime validation runs in Rust. See [Python example](../examples/python.py) and [wrapper API](../bindings/python/unilyrics/__init__.py).

## Java

Use JDK 17+; add the platform jar and its separate JSON-java dependency to the classpath. `load()` loads bundled Linux native resources:

```java
import org.tharukrj.unilyrics.LyricsModel;
import org.tharukrj.unilyrics.UniLyrics;

public class Main {
    public static void main(String[] args) {
        UniLyrics.load();
        var parsed = UniLyrics.parseTyped("[00:01.25]Hello", LyricsModel.Format.LRC);
        var converted = UniLyrics.export(parsed.document(),
            LyricsModel.Format.JSON, LyricsModel.Policy.STRICT);
        System.out.println(converted.output());
    }
}
```

Typed models are Java records. Map/list methods remain available. Use `loadSystem()` for app-managed engine/JNI libraries; an explicit bridge path is also supported. See [installation](INSTALLATION.md#java-and-kotlinjvm), [Java example](../examples/Example.java), and [API source](../bindings/java/src/main/java/org/tharukrj/unilyrics/UniLyrics.java).

## Kotlin

Add the Kotlin facade jar, Java SDK/platform jar, JSON-java and Kotlin standard library. Kotlin currently uses the JVM/JNI API, with Java's model records:

```kotlin
import org.tharukrj.unilyrics.Lyrics
import org.tharukrj.unilyrics.LyricsModel

fun main() {
    Lyrics.load()
    val parsed = Lyrics.parseTyped("[00:01.25]Hello", LyricsModel.Format.LRC)
    val converted = Lyrics.export(parsed.document(),
        LyricsModel.Format.JSON, LyricsModel.Policy.STRICT)
    println(converted.output())
}
```

See [Kotlin example](../examples/kotlin.kt) and [facade API](../bindings/kotlin/src/main/kotlin/org/tharukrj/unilyrics/Lyrics.kt). For Android, use matching Android native binaries and platform JSON rather than the Linux jar's native libraries or desktop JSON dependency. Follow the [Android guide](../bindings/android/README.md); API 29 installed-app checks pass; broader release coverage remains pending. This is not a Kotlin Multiplatform/Kotlin Native distribution.

## Go

The extracted native SDK includes a source module. Add a local replacement:

```go
require github.com/TharukRenuja/UniLyrics/bindings/go v0.0.0
replace github.com/TharukRenuja/UniLyrics/bindings/go => /path/to/unilyrics-native/bindings/go
```

Enable cgo and configure native link/runtime paths as described in [installation](INSTALLATION.md#go). Then:

```go
package main

import (
    "fmt"
    "log"
    lyrics "github.com/TharukRenuja/UniLyrics/bindings/go"
)

func main() {
    parsed, err := lyrics.Parse("[00:01.25]Hello", "lrc")
    if err != nil { log.Fatal(err) }
    converted, err := lyrics.Export(parsed.Document, "json", "strict")
    if err != nil { log.Fatal(err) }
    fmt.Println(converted.Output)
}
```

Results use typed Go structures and errors. A C compiler is needed to build the consumer; prebuilt native engines remove the need for Rust. See [Go example](../examples/go.go) and [module source](../bindings/go).

## Swift

Add the extracted `bindings/swift` as a local SwiftPM dependency, depend on its `UniLyrics` product, and supply native link/runtime paths from [installation](INSTALLATION.md#swift):

```swift
import UniLyrics

let parsed = try UniLyrics.parseTyped("[00:01.25]Hello", format: .lrc)
let converted = try UniLyrics.export(parsed.document, format: .json, policy: .strict)
print(converted.output)
```

Typed models support Codable; Foundation object APIs also exist. See [Swift example](../examples/swift.swift) and [package source](../bindings/swift). Linux Swift verification does not establish iOS/macOS support. Apple binary packaging, final linking and device/simulator checks remain pending.

## C

Use the extracted native SDK and its CMake target:

```cmake
find_package(UniLyrics 0.1 CONFIG REQUIRED)
target_link_libraries(my_app PRIVATE UniLyrics::unilyrics)
```

Configure with `-DCMAKE_PREFIX_PATH=/path/to/unilyrics-native`. Include the header, call with a valid UTF-8 byte buffer and free each returned pointer exactly once:

```c
#include <unilyrics.h>
#include <stdio.h>
#include <string.h>

int main(void) {
    const char *text = "[00:01.25]Hello";
    char *reply = unilyrics_parse((const uint8_t *)text, strlen(text), UNILYRICS_LRC);
    if (!reply) return 1;
    puts(reply);
    unilyrics_free(reply);
    return 0;
}
```

The reply is a JSON envelope. Use your app's JSON library to inspect `ok`, then `result.document` or `error`. Model inputs to other helpers are document JSON, not the entire parse reply. Do not use `free()` or retain a freed reply pointer. See [C example](../examples/c.c) and [public header](../bindings/c/unilyrics.h).

## C++

Use C++17 and link the `UniLyrics::cpp` CMake target. The header wrapper manages native response ownership and returns `std::string`:

```cpp
#include <unilyrics.hpp>
#include <iostream>

int main() {
    std::cout << unilyrics::parse("[00:01.25]Hello", UNILYRICS_LRC) << '\n';
}
```

Responses still contain the JSON envelope; inspect its success/error fields. The wrapper does not add a JSON model dependency or turn every core error into a C++ exception. See [C++ example](../examples/cpp.cpp) and [public wrapper](../bindings/cpp/unilyrics.hpp).

## Next steps

[API workflows](API.md) covers all operations. Detailed guides include native method names for [normalization](NORMALIZATION.md), [profiles](PROFILES.md), [preservation](PRESERVATION.md), and [custom registries](EXTENSIONS.md).

## C# and .NET

Use the .NET 10 project or local NuGet package described in [installation](INSTALLATION.md#c-and-net).

```csharp
using UniLyrics;
using var lyrics = new Lyrics();
var parsed = lyrics.Parse("[00:01.25]Hello", Format.Lrc);
Document document = parsed.Document;
var converted = lyrics.Export(document, Format.Plain, Policy.Compatible);
```

`Lyrics` exposes typed records, JSON attributes, structured `LyricsException` errors and the shared operations. Keep an instance while your application needs it, then dispose it. The package loads its native asset through .NET; external engine paths can be supplied explicitly. See the [complete example](../examples/csharp.cs) and [.NET guide](../bindings/dotnet/README.md).
