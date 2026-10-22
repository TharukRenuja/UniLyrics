use crate::{inspect, validate, Diagnostic, Document, Error, ParseResult, MAX_SAFE_TIME_US};

/// Shift existing explicit times without inferring missing boundaries or changing source order.
/// The input is never mutated; any out-of-range result rejects the whole operation.
pub fn normalize(document: &Document, offset_us: i64) -> Result<ParseResult, Error> {
    validate(document)?;
    if offset_us.unsigned_abs() > MAX_SAFE_TIME_US {
        return Err(Error::new(
            "INVALID_OFFSET",
            "Offset exceeds the JSON-safe microsecond range.",
        ));
    }
    let mut result = ParseResult {
        document: document.clone(),
        diagnostics: vec![],
    };
    if offset_us == 0 {
        return Ok(result);
    }
    let mut shifted = false;
    let mut shift = |time: &mut Option<u64>| -> Result<(), Error> {
        if let Some(value) = time {
            let next = i128::from(*value) + i128::from(offset_us);
            if !(0..=i128::from(MAX_SAFE_TIME_US)).contains(&next) {
                return Err(Error::new(
                    "INVALID_TIME",
                    "Timing offset would exceed the JSON-safe nonnegative range.",
                ));
            }
            *value = next as u64;
            shifted = true;
        }
        Ok(())
    };
    for cue in &mut result.document.cues {
        shift(&mut cue.start_us)?;
        shift(&mut cue.end_us)?;
        for segment in &mut cue.segments {
            shift(&mut segment.start_us)?;
            shift(&mut segment.end_us)?;
        }
    }
    for auxiliary in &mut result.document.auxiliary {
        for segment in &mut auxiliary.segments {
            shift(&mut segment.start_us)?;
            shift(&mut segment.end_us)?;
        }
    }
    validate(&result.document)?;
    if shifted {
        result.diagnostics.push(Diagnostic::new(
            "TIMING_SHIFTED",
            "Explicit cue, segment and auxiliary times were shifted by the requested offset.",
            None,
        ));
        if inspect(document)?.extensions {
            result.diagnostics.push(Diagnostic::new("EXTENSIONS_UNCHANGED", "Opaque attributes were retained unchanged; embedded timing or source provenance may need application-specific updates.", None));
        }
        if document
            .metadata
            .iter()
            .any(|entry| entry.key.eq_ignore_ascii_case("offset"))
        {
            result.diagnostics.push(Diagnostic::new(
                "OFFSET_METADATA_UNCHANGED",
                "Source offset metadata was retained verbatim and was not interpreted or consumed.",
                None,
            ));
        }
    }
    Ok(result)
}
