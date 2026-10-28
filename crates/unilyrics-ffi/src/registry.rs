use super::{guarded, input, output};
use std::ffi::{c_char, c_void, CString};
use std::panic::AssertUnwindSafe;
use std::sync::RwLock;
use unilyrics::{CustomFormat, Document, Error, ExportResult, ParseResult, Policy, Registry};

type Matches = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32;
type Parse = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> *mut c_char;
type Export = unsafe extern "C" fn(*mut c_void, *const u8, usize, i32) -> *mut c_char;

struct ForeignAdapter {
    context: *mut c_void,
    matches: Option<Matches>,
    parse: Option<Parse>,
    export: Option<Export>,
}

// Callers guarantee context lifetime and thread safety until all registry calls finish.
unsafe impl Send for ForeignAdapter {}
unsafe impl Sync for ForeignAdapter {}

unsafe fn callback_reply(pointer: *mut c_char) -> Result<serde_json::Value, Error> {
    if pointer.is_null() {
        return Err(Error::new(
            "INVALID_CALLBACK_RESULT",
            "Callback returned null.",
        ));
    }
    // Ownership transfers from the callback; only strings allocated by this library are valid.
    let owned = unsafe { CString::from_raw(pointer) };
    if owned.as_bytes().len() > unilyrics::MAX_INPUT_BYTES {
        return Err(Error::new(
            "INPUT_TOO_LARGE",
            "Callback reply exceeds 1 MiB.",
        ));
    }
    let reply: serde_json::Value = serde_json::from_slice(owned.as_bytes())
        .map_err(|e| Error::new("INVALID_CALLBACK_RESULT", e.to_string()))?;
    match reply.get("ok").and_then(serde_json::Value::as_bool) {
        Some(true) => reply
            .get("result")
            .cloned()
            .ok_or_else(|| Error::new("INVALID_CALLBACK_RESULT", "Callback result is missing.")),
        Some(false) => {
            let error: Error =
                serde_json::from_value(reply.get("error").cloned().unwrap_or_default())
                    .map_err(|e| Error::new("INVALID_CALLBACK_RESULT", e.to_string()))?;
            Err(error)
        }
        None => Err(Error::new(
            "INVALID_CALLBACK_RESULT",
            "Callback must return a normal UniLyrics JSON envelope.",
        )),
    }
}

impl CustomFormat for ForeignAdapter {
    fn matches(&self, text: &str) -> bool {
        self.matches.is_some_and(|callback| unsafe {
            callback(self.context, text.as_ptr(), text.len()) != 0
        })
    }
    fn parse(&self, text: &str) -> Result<ParseResult, Error> {
        let callback = self
            .parse
            .ok_or_else(|| Error::new("UNSUPPORTED_OPERATION", "Custom format has no parser."))?;
        let result = unsafe { callback_reply(callback(self.context, text.as_ptr(), text.len())) }?;
        serde_json::from_value(result)
            .map_err(|e| Error::new("INVALID_CALLBACK_RESULT", e.to_string()))
    }
    fn export(&self, document: &Document, policy: Policy) -> Result<ExportResult, Error> {
        let callback = self
            .export
            .ok_or_else(|| Error::new("UNSUPPORTED_OPERATION", "Custom format has no exporter."))?;
        let json = serde_json::to_string(document)
            .map_err(|e| Error::new("SERIALIZATION_ERROR", e.to_string()))?;
        let result = unsafe {
            callback_reply(callback(
                self.context,
                json.as_ptr(),
                json.len(),
                i32::from(policy == Policy::Strict),
            ))
        }?;
        serde_json::from_value(result)
            .map_err(|e| Error::new("INVALID_CALLBACK_RESULT", e.to_string()))
    }
}

pub struct NativeRegistry {
    inner: RwLock<Registry>,
}

/// Create a registry; free it once with `unilyrics_registry_free` after all calls finish.
#[no_mangle]
pub extern "C" fn unilyrics_registry_new() -> *mut NativeRegistry {
    Box::into_raw(Box::new(NativeRegistry {
        inner: RwLock::new(Registry::default()),
    }))
}

/// # Safety
/// `registry` must be null or a live handle from this library, with no active calls.
#[no_mangle]
pub unsafe extern "C" fn unilyrics_registry_free(registry: *mut NativeRegistry) {
    if !registry.is_null() {
        drop(unsafe { Box::from_raw(registry) });
    }
}

/// Register borrowed callbacks/context; returns 1 on success, 0 on failure.
///
/// # Safety
/// The handle and name buffer must be valid. Callbacks/context must remain valid and
/// thread-safe until the registry is freed and all active calls finish. Callbacks must
/// not unwind/longjmp across Rust. If nonnull, `error` must be a writable output slot;
/// a failure string is library-owned and must be freed with `unilyrics_free`.
#[no_mangle]
pub unsafe extern "C" fn unilyrics_registry_register(
    registry: *mut NativeRegistry,
    name: *const u8,
    length: usize,
    context: *mut c_void,
    matches: Option<Matches>,
    parse: Option<Parse>,
    export: Option<Export>,
    error: *mut *mut c_char,
) -> i32 {
    if !error.is_null() {
        unsafe { *error = std::ptr::null_mut() };
    }
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let registry = unsafe { registry.as_ref() }
            .ok_or_else(|| Error::new("INVALID_REGISTRY", "Null registry handle."))?;
        let name = unsafe { input(name, length) }?;
        registry
            .inner
            .write()
            .map_err(|_| Error::new("INTERNAL_ERROR", "Registry lock is poisoned."))?
            .register(
                name,
                ForeignAdapter {
                    context,
                    matches,
                    parse,
                    export,
                },
            )
    }))
    .unwrap_or_else(|_| {
        Err(Error::new(
            "INTERNAL_ERROR",
            "Registry registration failed.",
        ))
    });
    match result {
        Ok(()) => 1,
        Err(failure) => {
            if !error.is_null() {
                unsafe {
                    *error = output(serde_json::json!({"ok":false,"error":failure}).to_string())
                };
            }
            0
        }
    }
}

/// Parse (0), detect (1), or export (2) with scoped formats. Replies are owned JSON envelopes.
/// Empty format selects detection for parsing. Strict is 0 (compatible) or 1 (strict).
///
/// # Safety
/// The registry must be live for the entire call. Nonempty byte ranges must be readable
/// UTF-8 buffers. Contexts must remain alive, and callbacks must follow registration rules.
#[no_mangle]
pub unsafe extern "C" fn unilyrics_registry_call(
    registry: *mut NativeRegistry,
    data: *const u8,
    length: usize,
    operation: i32,
    format: *const u8,
    format_length: usize,
    strict: i32,
) -> *mut c_char {
    guarded(AssertUnwindSafe(|| {
        let registry = unsafe { registry.as_ref() }
            .ok_or_else(|| Error::new("INVALID_REGISTRY", "Null registry handle."))?;
        let snapshot = registry
            .inner
            .read()
            .map_err(|_| Error::new("INTERNAL_ERROR", "Registry lock is poisoned."))?
            .clone();
        let text = if length == 0 {
            ""
        } else {
            unsafe { input(data, length) }?
        };
        let format = if format_length == 0 {
            None
        } else {
            Some(unsafe { input(format, format_length) }?)
        };
        let policy = match strict {
            0 => Policy::Compatible,
            1 => Policy::Strict,
            _ => return Err(Error::new("INVALID_POLICY", "Unknown native policy code.")),
        };
        let result = match operation {
            0 => serde_json::to_value(snapshot.parse(text, format)?),
            1 => serde_json::to_value(snapshot.detect(text)?),
            2 => {
                let document: Document = serde_json::from_str(text)
                    .map_err(|e| Error::new("INVALID_JSON", e.to_string()))?;
                serde_json::to_value(snapshot.export(
                    &document,
                    format.ok_or_else(|| {
                        Error::new("UNKNOWN_FORMAT", "Export format is required.")
                    })?,
                    policy,
                )?)
            }
            _ => {
                return Err(Error::new(
                    "INVALID_OPERATION",
                    "Unknown registry operation.",
                ))
            }
        }
        .map_err(|e| Error::new("SERIALIZATION_ERROR", e.to_string()))?;
        Ok(serde_json::json!({"ok":true,"result":result}).to_string())
    }))
}

/// Build an export callback reply from UTF-8 output with an empty adapter loss list.
/// Registry round-trip verification still determines whether preservation is proven.
///
/// # Safety
/// Nonempty `data` must be a readable UTF-8 range. Free the result or transfer it once to a callback caller.
#[no_mangle]
pub unsafe extern "C" fn unilyrics_export_result(data: *const u8, length: usize) -> *mut c_char {
    guarded(|| {
        let text = if length == 0 {
            ""
        } else {
            unsafe { input(data, length) }?
        };
        Ok(serde_json::json!({"ok":true,"result":{"output":text,"losses":[]}}).to_string())
    })
}

/// Create an owned structured callback error with JSON escaping handled by the library.
///
/// # Safety
/// Nonempty code/message ranges must be readable UTF-8. Free or transfer the result once.
#[no_mangle]
pub unsafe extern "C" fn unilyrics_error_result(
    code: *const u8,
    code_length: usize,
    message: *const u8,
    message_length: usize,
) -> *mut c_char {
    guarded(|| {
        let code = if code_length == 0 {
            ""
        } else {
            unsafe { input(code, code_length) }?
        };
        let message = if message_length == 0 {
            ""
        } else {
            unsafe { input(message, message_length) }?
        };
        Err(Error::new(code, message))
    })
}

/// Copy a UTF-8 JSON callback envelope into a reply owned by this library.
/// The registry still validates the returned result's schema and canonical model.
///
/// # Safety
/// `data` must be a readable UTF-8 byte range. Free or transfer the result exactly once.
#[no_mangle]
pub unsafe extern "C" fn unilyrics_copy_reply(data: *const u8, length: usize) -> *mut c_char {
    guarded(|| {
        let text = if length == 0 {
            ""
        } else {
            unsafe { input(data, length) }?
        };
        let reply: serde_json::Value = serde_json::from_str(text)
            .map_err(|e| Error::new("INVALID_CALLBACK_RESULT", e.to_string()))?;
        match reply.get("ok").and_then(serde_json::Value::as_bool) {
            Some(true) if reply.get("result").is_some() => {}
            Some(false) => {
                let _: Error =
                    serde_json::from_value(reply.get("error").cloned().unwrap_or_default())
                        .map_err(|e| Error::new("INVALID_CALLBACK_RESULT", e.to_string()))?;
            }
            _ => {
                return Err(Error::new(
                    "INVALID_CALLBACK_RESULT",
                    "Expected a UniLyrics reply envelope.",
                ))
            }
        }
        Ok(text.to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Context {
        calls: AtomicUsize,
        registry: *mut NativeRegistry,
    }
    unsafe extern "C" fn matches(_: *mut c_void, bytes: *const u8, length: usize) -> i32 {
        i32::from(
            unsafe { input(bytes, length) }
                .unwrap()
                .starts_with("CUSTOM:"),
        )
    }
    unsafe extern "C" fn parse(
        context: *mut c_void,
        bytes: *const u8,
        length: usize,
    ) -> *mut c_char {
        let context = unsafe { &*(context as *const Context) };
        context.calls.fetch_add(1, Ordering::Relaxed);
        let text = unsafe { input(bytes, length) }.unwrap();
        if let Some(text) = text.strip_prefix("CUSTOM:") {
            unsafe { super::super::unilyrics_call(text.as_ptr(), text.len(), 0, 4, 0) }
        } else {
            unsafe {
                unilyrics_error_result(
                    b"BAD_HEADER".as_ptr(),
                    10,
                    b"Missing CUSTOM prefix".as_ptr(),
                    21,
                )
            }
        }
    }
    unsafe extern "C" fn export(
        _: *mut c_void,
        bytes: *const u8,
        length: usize,
        _: i32,
    ) -> *mut c_char {
        let text = format!("CUSTOM:{}", unsafe { input(bytes, length) }.unwrap());
        unsafe { unilyrics_export_result(text.as_ptr(), text.len()) }
    }
    unsafe extern "C" fn bad(_: *mut c_void, _: *const u8, _: usize) -> *mut c_char {
        output("not json".into())
    }
    unsafe extern "C" fn null(_: *mut c_void, _: *const u8, _: usize) -> *mut c_char {
        std::ptr::null_mut()
    }
    unsafe extern "C" fn reentrant(context: *mut c_void, _: *const u8, _: usize) -> *mut c_char {
        let context = unsafe { &*(context as *const Context) };
        let mut error = std::ptr::null_mut();
        let ok = unsafe {
            unilyrics_registry_register(
                context.registry,
                b"x-inside".as_ptr(),
                8,
                std::ptr::null_mut(),
                None,
                None,
                None,
                &mut error,
            )
        };
        assert_eq!(ok, 1);
        assert!(error.is_null());
        unsafe { super::super::unilyrics_call(b"hi".as_ptr(), 2, 0, 1, 0) }
    }
    unsafe fn decode(reply: *mut c_char) -> serde_json::Value {
        let value = serde_json::from_slice(unsafe { CStr::from_ptr(reply) }.to_bytes()).unwrap();
        unsafe { super::super::unilyrics_free(reply) };
        value
    }
    unsafe fn register(
        registry: *mut NativeRegistry,
        name: &str,
        context: *mut c_void,
        parser: Option<Parse>,
    ) -> serde_json::Value {
        let mut error = std::ptr::null_mut();
        let ok = unsafe {
            unilyrics_registry_register(
                registry,
                name.as_ptr(),
                name.len(),
                context,
                Some(matches),
                parser,
                Some(export),
                &mut error,
            )
        };
        if ok == 1 {
            assert!(error.is_null());
            serde_json::json!({"ok":true})
        } else {
            unsafe { decode(error) }
        }
    }
    unsafe fn call(
        registry: *mut NativeRegistry,
        text: &str,
        op: i32,
        format: &str,
        strict: i32,
    ) -> serde_json::Value {
        unsafe {
            decode(unilyrics_registry_call(
                registry,
                text.as_ptr(),
                text.len(),
                op,
                format.as_ptr(),
                format.len(),
                strict,
            ))
        }
    }

    #[test]
    fn native_callbacks_rich_corpus_and_reentrancy() {
        unsafe {
            let registry = unilyrics_registry_new();
            let mut context = Context {
                calls: AtomicUsize::new(0),
                registry,
            };
            let pointer = (&mut context as *mut Context).cast::<c_void>();
            assert_eq!(
                register(registry, "x-wrapped", pointer, Some(parse))["ok"],
                true
            );
            let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
                "../../unilyrics/tests/fixtures/conformance.json"
            ))
            .unwrap();
            for case in cases.into_iter().filter(|c| c.get("error").is_none()) {
                let format: unilyrics::Format =
                    serde_json::from_value(case["format"].clone()).unwrap();
                let text = case["input"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| case["document"].to_string());
                let original = unilyrics::parse(&text, Some(format)).unwrap().document;
                let json = serde_json::to_string(&original).unwrap();
                let exported = call(registry, &json, 2, "x-wrapped", 1);
                assert_eq!(exported["ok"], true, "{exported}");
                let custom = exported["result"]["output"].as_str().unwrap();
                assert_eq!(call(registry, custom, 1, "", 0)["result"], "x-wrapped");
                assert_eq!(
                    call(registry, custom, 0, "", 0)["result"]["document"],
                    serde_json::to_value(original).unwrap()
                );
            }
            let address = registry as usize;
            std::thread::scope(|scope| {
                for index in 0..4 {
                    scope.spawn(move || {
                        let handle = address as *mut NativeRegistry;
                        let name = format!("x-thread-{index}");
                        let mut error = std::ptr::null_mut();
                        assert_eq!(
                            unilyrics_registry_register(
                                handle,
                                name.as_ptr(),
                                name.len(),
                                std::ptr::null_mut(),
                                None,
                                None,
                                None,
                                &mut error
                            ),
                            1
                        );
                        assert!(error.is_null());
                        for _ in 0..10 {
                            assert_eq!(
                                call(handle, "CUSTOM:{\"version\":1,\"cues\":[]}", 0, "", 0)["ok"],
                                true
                            );
                        }
                    });
                }
            });
            assert!(context.calls.load(Ordering::Relaxed) > 0);
            assert_eq!(
                register(registry, "x-reentrant", pointer, Some(reentrant))["ok"],
                true
            );
            assert_eq!(call(registry, "hi", 0, "x-reentrant", 0)["ok"], true);
            unilyrics_registry_free(registry);
            unilyrics_registry_free(std::ptr::null_mut());
        }
    }

    #[test]
    fn copying_callback_replies_validates_and_owns_storage() {
        unsafe {
            for reply in [
                r#"{"ok":true,"result":{"output":"hello","losses":[]}}"#,
                r#"{"ok":false,"error":{"code":"CUSTOM","message":"failure"}}"#,
            ] {
                assert_eq!(
                    decode(unilyrics_copy_reply(reply.as_ptr(), reply.len())),
                    serde_json::from_str::<serde_json::Value>(reply).unwrap()
                );
            }
            for reply in [
                "not json",
                "{}",
                r#"{"ok":false,"error":{}}"#,
                r#"{"ok":true}"#,
            ] {
                assert_eq!(
                    decode(unilyrics_copy_reply(reply.as_ptr(), reply.len()))["error"]["code"],
                    "INVALID_CALLBACK_RESULT"
                );
            }
        }
    }

    #[test]
    fn native_failure_results_and_ownership() {
        unsafe {
            let registry = unilyrics_registry_new();
            assert_eq!(
                register(registry, "plain", std::ptr::null_mut(), Some(null))["error"]["code"],
                "INVALID_CUSTOM_FORMAT"
            );
            assert_eq!(
                register(registry, "x-null", std::ptr::null_mut(), Some(null))["ok"],
                true
            );
            assert_eq!(
                register(registry, "x-null", std::ptr::null_mut(), Some(null))["error"]["code"],
                "DUPLICATE_FORMAT"
            );
            assert_eq!(
                call(registry, "hi", 0, "x-null", 0)["error"]["code"],
                "INVALID_CALLBACK_RESULT"
            );
            assert_eq!(
                register(registry, "x-bad", std::ptr::null_mut(), Some(bad))["ok"],
                true
            );
            assert_eq!(
                call(registry, "hi", 0, "x-bad", 0)["error"]["code"],
                "INVALID_CALLBACK_RESULT"
            );
            assert_eq!(
                call(registry, "hi", 99, "", 0)["error"]["code"],
                "INVALID_OPERATION"
            );
            assert_eq!(
                call(registry, "hi", 0, "plain", 9)["error"]["code"],
                "INVALID_POLICY"
            );
            assert_eq!(
                call(std::ptr::null_mut(), "hi", 0, "plain", 0)["error"]["code"],
                "INVALID_REGISTRY"
            );
            assert_eq!(
                decode(unilyrics_error_result(
                    b"CUSTOM_FAILURE".as_ptr(),
                    14,
                    b"bad \"text\"".as_ptr(),
                    10
                ))["error"]["message"],
                "bad \"text\""
            );
            unilyrics_registry_free(registry);
        }
    }
}
