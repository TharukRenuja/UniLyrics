mod registry;

use std::ffi::{c_char, CString};

fn output(value: String) -> *mut c_char {
    CString::new(value)
        .expect("JSON escapes NUL characters")
        .into_raw()
}

fn guarded(
    action: impl FnOnce() -> Result<String, unilyrics::Error> + std::panic::UnwindSafe,
) -> *mut c_char {
    let result = std::panic::catch_unwind(action).unwrap_or_else(|_| {
        Err(unilyrics::Error::new(
            "INTERNAL_ERROR",
            "Core processing failed.",
        ))
    });
    output(match result {
        Ok(response) => response,
        Err(error) => serde_json::json!({"ok":false,"error":error}).to_string(),
    })
}

unsafe fn input<'a>(data: *const u8, length: usize) -> Result<&'a str, unilyrics::Error> {
    if data.is_null() {
        return Err(unilyrics::Error::new(
            "INVALID_BUFFER",
            "Null request buffer.",
        ));
    }
    if length > unilyrics::MAX_INPUT_BYTES {
        return Err(unilyrics::Error::new(
            "INPUT_TOO_LARGE",
            "Request exceeds 1 MiB.",
        ));
    }
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(data, length) })
        .map_err(|_| unilyrics::Error::new("INVALID_UTF8", "Request is not UTF-8."))
}

/// Process a UTF-8 JSON request. Free the returned string with `unilyrics_free`.
///
/// # Safety
/// `data` must point to `length` readable bytes for the duration of this call.
/// A null pointer is rejected. The library never retains the input pointer.
#[no_mangle]
pub unsafe extern "C" fn unilyrics_process(data: *const u8, length: usize) -> *mut c_char {
    guarded(|| Ok(unilyrics::process(unsafe { input(data, length)? })))
}

fn native_format(format: i32) -> Result<serde_json::Value, unilyrics::Error> {
    Ok(match format {
        0 => serde_json::Value::Null,
        1 => serde_json::json!("plain"),
        2 => serde_json::json!("lrc"),
        3 => serde_json::json!("enhanced_lrc"),
        4 => serde_json::json!("json"),
        5 => serde_json::json!("ttml"),
        6 => serde_json::json!("ttml_lyrics"),
        7 => serde_json::json!("srt"),
        8 => serde_json::json!("webvtt"),
        _ => {
            return Err(unilyrics::Error::new(
                "INVALID_FORMAT",
                "Unknown native format code.",
            ))
        }
    })
}

/// Call parse (0), detect (1), export (2), validate (3), or inspect (4) without constructing a request.
/// Formats: auto (0), plain (1), LRC (2), Enhanced LRC (3), JSON (4), TTML (5), music TTML (6), SRT (7), WebVTT (8).
/// `strict` is 0 for compatible or 1 for strict. Replies use the usual JSON envelope.
///
/// # Safety
/// Nonempty input must point to `length` readable UTF-8 bytes. For export/validate,
/// input is a canonical document JSON string. Release the result with `unilyrics_free`.
#[no_mangle]
pub unsafe extern "C" fn unilyrics_call(
    data: *const u8,
    length: usize,
    operation: i32,
    format: i32,
    strict: i32,
) -> *mut c_char {
    guarded(|| {
        let text = if length == 0 {
            ""
        } else {
            unsafe { input(data, length)? }
        };
        let format = native_format(format)?;
        if !(0..=1).contains(&strict) {
            return Err(unilyrics::Error::new(
                "INVALID_POLICY",
                "Unknown native policy code.",
            ));
        }
        let request = match operation {
            0 => serde_json::json!({"op":"parse","input":text,"format":format}),
            1 => serde_json::json!({"op":"detect","input":text}),
            2..=4 => {
                let document: serde_json::Value = serde_json::from_str(text)
                    .map_err(|e| unilyrics::Error::new("INVALID_JSON", e.to_string()))?;
                if operation == 2 {
                    serde_json::json!({"op":"export","document":document,"format":format,"policy":if strict == 1 {"strict"} else {"compatible"}})
                } else {
                    serde_json::json!({"op":if operation == 3 {"validate"} else {"inspect"},"document":document})
                }
            }
            _ => {
                return Err(unilyrics::Error::new(
                    "INVALID_OPERATION",
                    "Unknown native operation code.",
                ))
            }
        };
        Ok(unilyrics::process(&request.to_string()))
    })
}

/// Normalize canonical document JSON with a signed timing offset in microseconds.
/// Replies use the usual envelope and must be released with `unilyrics_free`.
///
/// # Safety
/// `data` must point to `length` readable UTF-8 bytes for this call.
#[no_mangle]
pub unsafe extern "C" fn unilyrics_normalize(
    data: *const u8,
    length: usize,
    offset_us: i64,
) -> *mut c_char {
    guarded(|| {
        let document: serde_json::Value = serde_json::from_str(unsafe { input(data, length)? })
            .map_err(|e| unilyrics::Error::new("INVALID_JSON", e.to_string()))?;
        Ok(unilyrics::process(
            &serde_json::json!({"op":"normalize", "document":document, "offset_us":offset_us})
                .to_string(),
        ))
    })
}

unsafe fn profile_request(
    data: *const u8,
    length: usize,
    options: *const u8,
    options_length: usize,
) -> Result<serde_json::Value, unilyrics::Error> {
    let document: serde_json::Value = serde_json::from_str(unsafe { input(data, length)? })
        .map_err(|e| unilyrics::Error::new("INVALID_JSON", e.to_string()))?;
    let options: serde_json::Value = if options_length == 0 {
        serde_json::json!({})
    } else {
        serde_json::from_str(unsafe { input(options, options_length)? })
            .map_err(|e| unilyrics::Error::new("INVALID_JSON", e.to_string()))?
    };
    Ok(serde_json::json!({"op":"project", "document":document, "options":options}))
}

/// Project canonical document JSON using JSON profile options; empty options select full.
/// # Safety
/// Buffers must be readable for their lengths. Empty options may be null.
/// Free the owned response with `unilyrics_free`.
#[no_mangle]
pub unsafe extern "C" fn unilyrics_project(
    data: *const u8,
    length: usize,
    options: *const u8,
    options_length: usize,
) -> *mut c_char {
    guarded(|| {
        Ok(unilyrics::process(
            &unsafe { profile_request(data, length, options, options_length)? }.to_string(),
        ))
    })
}

/// Export canonical document JSON using a profile, format code and 0/1 strict policy.
/// # Safety
/// Buffers must be readable for their lengths. Empty options may be null.
/// Free the owned response with `unilyrics_free`.
#[no_mangle]
pub unsafe extern "C" fn unilyrics_export_profile(
    data: *const u8,
    length: usize,
    format: i32,
    strict: i32,
    options: *const u8,
    options_length: usize,
) -> *mut c_char {
    guarded(|| {
        let format = native_format(format)?;
        if !(0..=1).contains(&strict) {
            return Err(unilyrics::Error::new(
                "INVALID_POLICY",
                "Unknown native policy code.",
            ));
        }
        let mut request = unsafe { profile_request(data, length, options, options_length)? };
        request["op"] = serde_json::json!("export_profile");
        request["format"] = format;
        request["policy"] = serde_json::json!(if strict == 1 { "strict" } else { "compatible" });
        Ok(unilyrics::process(&request.to_string()))
    })
}

/// Export canonical JSON plus an owned preservation sidecar in the result envelope.
/// # Safety
/// Buffers must be readable for their lengths. Empty options may be null.
/// Release the returned response with `unilyrics_free`.
#[no_mangle]
pub unsafe extern "C" fn unilyrics_export_preserved(
    data: *const u8,
    length: usize,
    format: i32,
    options: *const u8,
    options_length: usize,
) -> *mut c_char {
    guarded(|| {
        let format = native_format(format)?;
        let mut request = unsafe { profile_request(data, length, options, options_length)? };
        request["op"] = serde_json::json!("export_preserved");
        request["format"] = format;
        Ok(unilyrics::process(&request.to_string()))
    })
}

/// Restore exact output using its canonical sidecar JSON and target format code.
/// # Safety
/// Buffers must be readable for their lengths. Empty output may be null;
/// sidecar must be a non-null JSON buffer. Release the response with `unilyrics_free`.
#[no_mangle]
pub unsafe extern "C" fn unilyrics_restore_preserved(
    data: *const u8,
    length: usize,
    format: i32,
    sidecar: *const u8,
    sidecar_length: usize,
) -> *mut c_char {
    guarded(|| {
        let format = native_format(format)?;
        let text = if length == 0 {
            ""
        } else {
            unsafe { input(data, length)? }
        };
        let sidecar: serde_json::Value =
            serde_json::from_str(unsafe { input(sidecar, sidecar_length)? })
                .map_err(|e| unilyrics::Error::new("INVALID_JSON", e.to_string()))?;
        Ok(unilyrics::process(&serde_json::json!({"op":"restore_preserved", "input":text, "format":format, "sidecar":sidecar}).to_string()))
    })
}

/// Release exactly one string returned by `unilyrics_process`. Null is allowed.
///
/// # Safety
/// The non-null pointer must be an unfreed result from this library instance.
#[no_mangle]
pub unsafe extern "C" fn unilyrics_free(value: *mut c_char) {
    if !value.is_null() {
        drop(unsafe { CString::from_raw(value) });
    }
}

// The WASM loader copies requests into linear memory; native callers own their buffers.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn unilyrics_alloc(length: usize) -> *mut u8 {
    if length == 0 || length > unilyrics::MAX_INPUT_BYTES {
        return std::ptr::null_mut();
    }
    Box::into_raw(vec![0u8; length].into_boxed_slice()) as *mut u8
}

/// # Safety
/// `data` and `length` must be the exact unfreed allocation from `unilyrics_alloc`.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub unsafe extern "C" fn unilyrics_free_input(data: *mut u8, length: usize) {
    if !data.is_null() {
        drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(data, length)) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;
    #[test]
    fn protocol_and_ownership() {
        let input = br#"{"op":"parse","input":"[00:01.00]hello","format":"lrc"}"#;
        unsafe {
            let result = unilyrics_process(input.as_ptr(), input.len());
            assert!(CStr::from_ptr(result).to_str().unwrap().contains("hello"));
            unilyrics_free(result);
            let error = unilyrics_process(std::ptr::null(), 0);
            assert!(CStr::from_ptr(error)
                .to_str()
                .unwrap()
                .contains("INVALID_BUFFER"));
            unilyrics_free(error);
            let error = unilyrics_process([255u8].as_ptr(), 1);
            assert!(CStr::from_ptr(error)
                .to_str()
                .unwrap()
                .contains("INVALID_UTF8"));
            unilyrics_free(error);
            unilyrics_free(std::ptr::null_mut());
        }
    }

    #[test]
    fn direct_operations_and_invalid_parameters() {
        let document = br#"{"version":1,"cues":[],"metadata":[],"attributes":{}}"#;
        for (text, operation, format, strict, expected) in [
            (b"WEBVTT\n\n00:01.250 --> 00:02.000\nvoice".as_slice(), 0, 8, 0, "1250000"),
            (b"1\n00:00:01,250 --> 00:00:02,000\nvoice".as_slice(), 0, 7, 0, "1250000"),
            (br#"<tt xmlns="http://www.w3.org/ns/ttml"><body><p begin="1s"><span begin="250ms">x</span></p></body></tt>"#.as_slice(), 0, 5, 0, "1250000"),
            (br#"<tt xmlns="http://www.w3.org/ns/ttml"><body><p begin="1s"><span begin="1250ms">x</span></p></body></tt>"#.as_slice(), 0, 6, 0, "1250000"),
            ("[00:01]hi".as_bytes(), 0, 2, 0, "1000000"),
            ("[00:01]hi".as_bytes(), 1, 0, 0, "lrc"),
            (document.as_slice(), 2, 1, 0, "output"),
            (document.as_slice(), 3, 0, 0, "valid"),
            (document.as_slice(), 4, 0, 0, "auxiliary_kinds"),
            (b"".as_slice(), 0, 1, 0, "document"),
            (b"".as_slice(), 99, 0, 0, "INVALID_OPERATION"),
            (b"".as_slice(), 0, 99, 0, "INVALID_FORMAT"),
            (b"".as_slice(), 0, 0, 99, "INVALID_POLICY"),
            (b"broken".as_slice(), 2, 1, 0, "INVALID_JSON"),
            ([255u8].as_slice(), 0, 1, 0, "INVALID_UTF8"),
        ] {
            unsafe {
                let pointer = if text.is_empty() {
                    std::ptr::null()
                } else {
                    text.as_ptr()
                };
                let response = unilyrics_call(pointer, text.len(), operation, format, strict);
                let response_text = CStr::from_ptr(response).to_str().unwrap().to_owned();
                unilyrics_free(response);
                assert!(response_text.contains(expected), "{response_text}");
            }
        }
    }
}
