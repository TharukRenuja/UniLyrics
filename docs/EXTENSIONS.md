# Custom format adapters

All ten SDK targets (Rust, JavaScript, TypeScript, Python, Go, Java, Kotlin, Swift, C and C++) support application-owned custom parsers and exporters through scoped registries. The C++ wrapper manages native registry and callback ownership with RAII. A callback cannot be registered by sending executable code through the native/WASM JSON protocol.

## Registration and dispatch

Register an adapter under a name beginning with `x-`, followed by lowercase ASCII letters, digits, `-` or `_`. Total length is 3–64 bytes. Duplicate registrations fail instead of replacing an existing adapter. Registries are independent, and built-in formats cannot be overridden. Cloned Rust registries have independent registration maps but share the registered adapter instances.

A registry routes built-in names to the existing shared parsers/exporters. For custom names, it calls the registered adapter. An unregistered name returns `UNKNOWN_FORMAT`. The free core functions keep their existing behavior; registration has no global effect.

Adapters can implement `parse`, `export`, and an optional `matches` detector. Default parse/export methods return `UNSUPPORTED_OPERATION`; the default detector returns false. Explicit format selection does not invoke detectors. During detection, two custom matches, or a custom match together with a structured built-in format, produce `AMBIGUOUS_FORMAT`. A single custom match may replace the plain-text fallback or recover an otherwise unrecognized/malformed built-in input. Select a format explicitly to resolve ambiguity.

## Validation and preservation

Registries enforce the core's 1 MiB text-input/output limit and validate every custom parsed document. Host SDK calls also share the 1 MiB encoded-request budget: verification includes the original document, export result and reparsed document, so their combined encoding can lower the effective payload limit compared with direct Rust calls. It validates documents before invoking custom exporters. Callbacks receive the canonical model, including rich vocals, auxiliary relationships, metadata and arbitrary attributes.

The exporter callback returns `ExportResult` and receives the requested policy. The registry additionally reparses its output using that same adapter and validates the resulting document:

- An equal canonical document adds no verification loss.
- A different document adds `CUSTOM_ROUNDTRIP_CHANGED`.
- Missing/failed parsing or an invalid returned document adds `CUSTOM_ROUNDTRIP_UNVERIFIED`.

Compatible export returns the output with these diagnostics and the adapter's own losses. Strict export rejects any reported or verification loss with `LOSSY_CONVERSION`. An exporter-only adapter therefore cannot claim a verified strict round trip. An empty loss list from a callback alone does not establish preservation.

Verification compares the canonical model. It does not establish conformance with an external proprietary format specification, preserve original byte spelling, or prove how a third-party player will render output.

## Example

```rust
use unilyrics::{CustomFormat, Document, Error, ExportResult, Format, ParseResult, Policy, Registry};

struct WrappedJSON;
impl CustomFormat for WrappedJSON {
    fn matches(&self, input: &str) -> bool {
        input.starts_with("CUSTOM\n")
    }
    fn parse(&self, input: &str) -> Result<ParseResult, Error> {
        let json = input.strip_prefix("CUSTOM\n")
            .ok_or_else(|| Error::new("BAD_CUSTOM_HEADER", "Missing CUSTOM header."))?;
        unilyrics::parse(json, Some(Format::Json))
    }
    fn export(&self, document: &Document, _: Policy) -> Result<ExportResult, Error> {
        let mut result = unilyrics::export(document, Format::Json, Policy::Strict)?;
        result.output = format!("CUSTOM\n{}", result.output);
        Ok(result)
    }
}

fn main() -> Result<(), Error> {
    let mut registry = Registry::default();
    registry.register("x-wrapped-json", WrappedJSON)?;
    let document = Document::default();
    let encoded = registry.export(&document, "x-wrapped-json", Policy::Strict)?;
    assert_eq!(registry.parse(&encoded.output, None)?.document, document);
    Ok(())
}
```

Rust adapters must implement `Send + Sync`. All adapters are trusted application code and run synchronously in-process. The SDK does not sandbox callback CPU/memory usage, catch application panics, load plugins from disk, or fetch code. Applications own callback dependencies and any side effects. Registry methods borrow documents immutably; the core does not mutate them.


## JavaScript and TypeScript

With an initialized `sdk`, use the exported `Registry` class. Adapters may provide synchronous `matches`, `parse`, and `export` methods. Async callbacks are unsupported. Methods retain their adapter's `this` binding.

```javascript
import {Registry, UniLyricsError} from "unilyrics";
const registry = new Registry(sdk);
registry.register("x-wrapped-json", {
  matches: input => input.startsWith("CUSTOM:"),
  parse(input) {
    if (!input.startsWith("CUSTOM:")) throw new UniLyricsError("BAD_CUSTOM_HEADER", "Missing CUSTOM prefix");
    return sdk.parse(input.slice("CUSTOM:".length), "json");
  },
  export(document, policy) {
    const result = sdk.export(document, "json", policy);
    return {...result, output: "CUSTOM:" + result.output};
  },
});
const original = sdk.parse("Hello", "plain").document;
const encoded = registry.export(original, "x-wrapped-json", "strict");
const parsed = registry.parse(encoded.output);
```

The npm package includes `CustomFormat` and `RegistryFormat` TypeScript declarations. Built-in operations may be called through the registry by their ordinary format names. Use the underlying `sdk` for normalization, feature inspection and validation.

## Python

Use `Registry(sdk)` and optional keyword callbacks `parser`, `exporter`, and `detector`. Parser/exporter callbacks return the standard typed-dictionary result shapes. A detector returns a boolean. Async callbacks are unsupported.

```python
from unilyrics import Registry, UniLyricsError
registry = Registry(sdk)

def parse_custom(text):
    if not text.startswith("CUSTOM:"):
        raise UniLyricsError("BAD_CUSTOM_HEADER", "Missing CUSTOM prefix")
    return sdk.parse(text[len("CUSTOM:"):], "json")

def export_custom(document, policy):
    result = sdk.export(document, "json", policy)
    result["output"] = "CUSTOM:" + result["output"]
    return result

registry.register("x-wrapped-json", parser=parse_custom,
                  exporter=export_custom, detector=lambda text: text.startswith("CUSTOM:"))
original = sdk.parse("Hello", "plain")["document"]
encoded = registry.export(original, "x-wrapped-json", "strict")
parsed = registry.parse(encoded["output"])
```

Host exporters receive an independent canonical document, and callback results are serialized and validated into independent values before verification. Changing an exporter argument cannot change the caller's original document through that argument. Application state captured by callbacks remains the application's responsibility. Primary parse/export/detector exceptions propagate; failures during export reparsing become `CUSTOM_ROUNDTRIP_UNVERIFIED` losses. Detector return types must be boolean.

## Go

The zero value of `Registry` is usable. `Adapter` holds optional synchronous `Matches`, `Parse`, and `Export` functions. An empty format string selects detection, and an empty export policy selects compatible mode. Nil diagnostic/loss slices in callback results mean empty lists; the engine returns canonical result shapes.

```go
import (
    "strings"
    lyrics "github.com/TharukRenuja/UniLyrics/bindings/go"
)

func customRegistry() (*lyrics.Registry, error) {
    var registry lyrics.Registry
    err := registry.Register("x-wrapped-json", lyrics.Adapter{
        Matches: func(input string) bool { return strings.HasPrefix(input, "CUSTOM:") },
        Parse: func(input string) (lyrics.ParseResult, error) {
            input, ok := strings.CutPrefix(input, "CUSTOM:")
            if !ok {
                return lyrics.ParseResult{}, &lyrics.Error{Code: "BAD_CUSTOM_HEADER", Message: "Missing CUSTOM prefix"}
            }
            return lyrics.Parse(input, "json")
        },
        Export: func(document lyrics.Document, policy string) (lyrics.ExportResult, error) {
            result, err := lyrics.Export(document, "json", policy)
            result.Output = "CUSTOM:" + result.Output
            return result, err
        },
    })
    return &registry, err
}
```

Use `registry.Parse(input, "")`, `registry.Detect(input)`, and `registry.Export(document, "x-wrapped-json", "strict")`. Built-in format names dispatch to the existing SDK functions. Primary callback errors propagate unchanged; reparse errors cause shared unverifiable-output diagnostics. Callback panics are not recovered.

Registration and lookup are safe concurrently. Detection snapshots registered callbacks before invoking them; no registry lock is held while callbacks execute, so a callback may register another format. Callbacks may run concurrently and must synchronize their own captured state. Do not copy a registry after first use. Export callbacks receive independent nested document values, and the checked export result is snapshotted before reparsing.

## Java and Kotlin

Both JVM SDKs use `org.tharukrj.unilyrics.Registry` and its typed `Registry.Adapter` interface. Load the native engine before invoking registry operations. Adapter methods have defaults: `matches` returns false, and missing parse/export methods throw `UNSUPPORTED_OPERATION`. `parse(input)` detects the format; `parse(input, name)` selects it explicitly. Export defaults to compatible policy.

```java
import org.tharukrj.unilyrics.UniLyrics;
import org.tharukrj.unilyrics.LyricsModel;
import org.tharukrj.unilyrics.Registry;

UniLyrics.load();
var registry = new Registry();
registry.register("x-wrapped-json", new Registry.Adapter() {
    public boolean matches(String input) { return input.startsWith("CUSTOM:"); }
    public LyricsModel.ParseResult parse(String input) {
        if (!matches(input)) throw new UniLyrics.LyricsException("BAD_CUSTOM_HEADER", "Missing CUSTOM prefix");
        return UniLyrics.parseTyped(input.substring("CUSTOM:".length()), LyricsModel.Format.JSON);
    }
    public LyricsModel.ExportResult export(LyricsModel.Document document, LyricsModel.Policy policy) {
        var result = UniLyrics.export(document, LyricsModel.Format.JSON, policy);
        return new LyricsModel.ExportResult("CUSTOM:" + result.output(), result.losses());
    }
});
var original = UniLyrics.parseTyped("Hello", LyricsModel.Format.PLAIN).document();
var encoded = registry.export(original, "x-wrapped-json", LyricsModel.Policy.STRICT);
var parsed = registry.parse(encoded.output());
```

Kotlin uses the same registry and records, with an object expression for the adapter:

```kotlin
import org.tharukrj.unilyrics.Lyrics
import org.tharukrj.unilyrics.LyricsModel
import org.tharukrj.unilyrics.Registry

Lyrics.load()
val registry = Registry()
registry.register("x-wrapped-json", object : Registry.Adapter {
    override fun matches(input: String) = input.startsWith("CUSTOM:")
    override fun parse(input: String): LyricsModel.ParseResult {
        require(matches(input)) { "Missing CUSTOM prefix" }
        return Lyrics.parseTyped(input.removePrefix("CUSTOM:"), LyricsModel.Format.JSON)
    }
    override fun export(document: LyricsModel.Document, policy: LyricsModel.Policy): LyricsModel.ExportResult {
        val result = Lyrics.export(document, LyricsModel.Format.JSON, policy)
        return LyricsModel.ExportResult("CUSTOM:" + result.output(), result.losses())
    }
})
val original = Lyrics.parseTyped("Hello", LyricsModel.Format.PLAIN).document()
val encoded = registry.export(original, "x-wrapped-json", LyricsModel.Policy.STRICT)
val parsed = registry.parse(encoded.output())
```

Registration and lookups are synchronized, while detection snapshots and invokes adapters outside the registry lock. Callbacks may run concurrently and must protect their own mutable state. Export callbacks receive independent mutable collection values; callback result collections are independently validated before reparsing. Standard record fields and callback results must be nonnull except documented optional model fields. Primary runtime exceptions propagate; runtime exceptions during export reparsing produce unverifiable-output losses. JVM `Error` instances are not swallowed.

## Swift

Use `LyricsRegistry` and `LyricsAdapter` with synchronous `@Sendable` closures. Missing callbacks report `UNSUPPORTED_OPERATION`; an omitted detector does not match. The API uses the existing typed Codable models. A nil parse format selects detection, and export defaults to compatible policy.

```swift
import UniLyrics

let registry = LyricsRegistry()
try registry.register("x-wrapped-json", adapter: LyricsAdapter(
    matches: { $0.hasPrefix("CUSTOM:") },
    parse: { input in
        guard input.hasPrefix("CUSTOM:") else {
            throw UniLyricsError(code: "BAD_CUSTOM_HEADER", message: "Missing CUSTOM prefix")
        }
        return try UniLyrics.parseTyped(String(input.dropFirst("CUSTOM:".count)), format: .json)
    },
    export: { document, policy in
        var result = try UniLyrics.export(document, format: .json, policy: policy)
        result.output = "CUSTOM:" + result.output
        return result
    }))
let original = try UniLyrics.parseTyped("Hello", format: .plain).document
let encoded = try registry.export(original, format: "x-wrapped-json", policy: .strict)
let parsed = try registry.parse(encoded.output)
```

The registry protects its callback dictionary with an `NSLock`, snapshots detection entries, and invokes callbacks outside the lock. It is Sendable with that explicit synchronization; callbacks may execute concurrently and must synchronize their captured state. Swift's document/result value semantics protect caller values when a callback makes mutable local copies. Typed requests go directly through JSONEncoder/JSONDecoder and preserve the existing signed/unsigned integer attribute support. Primary thrown errors propagate; failed export reparsing is reported as unverified. Fatal errors/traps are not caught. Callbacks are retained; use weak captures when capturing their registry or owner would create an ARC cycle. See [platform requirements](PLATFORMS.md) for Swift distribution availability.

## C

The C SDK exposes an opaque `unilyrics_registry` and three borrowed callback types. The parse/export callbacks return ordinary JSON reply envelopes allocated by the **same library instance**. The SDK takes ownership and frees that reply exactly once, including error/malformed-response paths. Return it unchanged: never return static strings, `malloc` buffers or replies you already freed/reallocated. Callback replies have a 1 MiB encoded limit.

- `unilyrics_registry_new/free` create/destroy the handle; null destruction is allowed.
- `unilyrics_registry_register` borrows context and callbacks, returning 1/0. Its optional error slot receives an owned JSON error reply, or NULL on success. Free an old reply before reusing its output slot. Failed registration retains nothing.
- `unilyrics_registry_call` uses operation 0=parse, 1=detect, 2=export. Empty parse format means detection. Export input is canonical document JSON. Strict is 0/1. Free every returned operation reply.
- `unilyrics_export_result` creates an owned export-result envelope from UTF-8 output with an empty adapter loss list. Shared round-trip verification still determines whether strict conversion is acceptable.
- `unilyrics_error_result` creates an owned error envelope from UTF-8 code/message byte ranges, handling JSON escaping.

Context is not owned or destroyed by the SDK. Keep it and callback code alive until the handle is destroyed and all active operations have returned. Callbacks may run concurrently and must synchronize shared context. Registration is synchronized; operation snapshots release the lock before invoking callbacks, so reentrant registration is supported. Never destroy a handle during/from an active operation. No exceptions or longjmp may cross the callback boundary.

Example callbacks wrapping canonical JSON with a custom prefix:

```c
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <unilyrics.h>

static int32_t matches(void *context, const uint8_t *input, size_t length) {
    (void)context;
    return length >= 7 && memcmp(input, "CUSTOM:", 7) == 0;
}
static char *parse_custom(void *context, const uint8_t *input, size_t length) {
    if (!matches(context, input, length)) {
        const char *code = "BAD_HEADER", *message = "Missing CUSTOM prefix";
        return unilyrics_error_result((const uint8_t *)code, strlen(code), (const uint8_t *)message, strlen(message));
    }
    return unilyrics_parse(input + 7, length - 7, UNILYRICS_JSON);
}
static char *export_custom(void *context, const uint8_t *document, size_t length, int32_t strict) {
    (void)context;
    (void)strict;
    uint8_t *buffer = malloc(length + 7);
    if (!buffer) return NULL;
    memcpy(buffer, "CUSTOM:", 7);
    memcpy(buffer + 7, document, length);
    char *reply = unilyrics_export_result(buffer, length + 7);
    free(buffer);
    return reply;
}
```

Create a handle, then register `"x-wrapped-json"` with these callbacks and an optional context pointer. Match callbacks return zero/nonzero and receive borrowed byte ranges valid only for that invocation. Parser/exporter callbacks can also return replies from the existing `unilyrics_process`, `unilyrics_parse`, or `unilyrics_export` functions. To report detailed adapter losses, build the standard `validate_custom_export` request with the application's JSON tooling; its reply already uses the required ownership convention.

The Rust-backed registry validates models, rejects duplicate/built-in overrides and ambiguous detection, and independently verifies the custom export by reparsing it. A null or malformed callback reply is an explicit `INVALID_CALLBACK_RESULT`. Application-owned unsafe pointers remain the caller's responsibility; the SDK cannot validate dangling pointers or recover from undefined native behavior.

## C++

`unilyrics::registry` is a noncopyable, movable C++17 RAII owner. Its `unilyrics::adapter` holds optional `std::function` callbacks. Registered callbacks and captured owning values stay alive until registry destruction, including after a move. Failed registration releases its new callback context and throws `std::runtime_error` containing the native JSON error. A moved-from registry rejects operations with `std::logic_error`.

```cpp
#include <unilyrics.hpp>

unilyrics::registry registry;
registry.register_format("x-wrapped-json", {
    [](std::string_view input) { return input.substr(0, 7) == "CUSTOM:"; },
    [](std::string_view input) {
        if (input.substr(0, 7) != "CUSTOM:")
            return unilyrics::error_result("BAD_HEADER", "Missing CUSTOM prefix");
        return unilyrics::parse(input.substr(7), UNILYRICS_JSON);
    },
    [](std::string_view document, bool) {
        return unilyrics::export_result("CUSTOM:" + std::string(document));
    }
});
const std::string document = R"({"version":1,"cues":[]})";
auto encoded = registry.export_lyrics(document, "x-wrapped-json", true);
auto parsed = registry.parse("CUSTOM:" + document);
```

Parse/export callbacks return a complete ordinary JSON reply envelope as `std::string`, using the existing helpers or the application's JSON library. `export_result` and `error_result` handle output/message escaping. The trampoline copies that string into library-owned storage via `unilyrics_copy_reply`; the native registry validates and consumes it exactly once. Callers never transfer a C++ allocation to Rust. Normal operations return JSON envelopes like the existing C++ SDK; the application may use its chosen JSON library to inspect results.

No callback exception unwinds through C/Rust. Parse/export exceptions become `CUSTOM_CALLBACK_ERROR` envelopes; a failed export reparse still causes shared unverifiable-output diagnostics. Detector exceptions cannot be encoded in a boolean return, so a thread-local call frame records them and rethrows after the native operation returns. Nested calls preserve their own exception frames, and all native replies are cleaned up before rethrowing.

Concurrent registration and operations are supported; callbacks execute outside registry locks and must synchronize their captured state. Do not move or destroy a registry during active calls. Captured references remain application-owned and are not rewritten by moving the registry. The native handle is destroyed before the owned callback contexts. Installed `UniLyrics::cpp` links `Threads::Threads`; manual compiler users should enable threading (for example `-pthread` on Linux).

## Shared callback support protocol

Host wrappers call three stateless operations in Rust:

| Operation | Request fields | Result |
|---|---|---|
| `validate_custom_parse` | `result` (parse result) | Validated canonical parse result |
| `validate_custom_export` | `result` (export result) | Checked export result, before callback reparsing |
| `verify_custom_export` | `document`, `result`, optional `roundtrip` document, optional `policy` | Export result with shared preservation diagnostics, or strict rejection |

Unknown fields in callback result envelopes are rejected. A missing, null or invalid `roundtrip` cannot establish preservation. The SDK host owns callback execution and supplies the reparse; Rust validates and compares the supplied canonical models. This is not proof against a dishonest host or conformance certification for a proprietary format. No callback functions, code, or plugin modules cross the JSON boundary.
