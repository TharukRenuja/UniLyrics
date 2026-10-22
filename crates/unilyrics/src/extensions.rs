use crate::{
    check_size, validate, Diagnostic, Document, Error, ExportResult, Format, ParseResult, Policy,
};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Application-owned adapter. Callbacks run locally and are trusted application code.
/// Missing parser/exporter implementations return a structured unsupported-operation error.
pub trait CustomFormat: Send + Sync {
    fn matches(&self, _input: &str) -> bool {
        false
    }
    fn parse(&self, _input: &str) -> Result<ParseResult, Error> {
        Err(Error::new(
            "UNSUPPORTED_OPERATION",
            "This custom format has no parser.",
        ))
    }
    fn export(&self, _document: &Document, _policy: Policy) -> Result<ExportResult, Error> {
        Err(Error::new(
            "UNSUPPORTED_OPERATION",
            "This custom format has no exporter.",
        ))
    }
}

/// Scoped format registration; built-in formats and other registries remain unchanged.
///
/// ```
/// use unilyrics::{CustomFormat, Error, Format, ParseResult, Registry};
/// struct CustomText;
/// impl CustomFormat for CustomText {
///     fn parse(&self, input: &str) -> Result<ParseResult, Error> {
///         unilyrics::parse(input, Some(Format::Plain))
///     }
/// }
/// let mut registry = Registry::default();
/// registry.register("x-text", CustomText)?;
/// let parsed = registry.parse("Hello", Some("x-text"))?;
/// assert_eq!(parsed.document.cues[0].text(), "Hello");
/// # Ok::<(), Error>(())
/// ```
#[derive(Clone, Default)]
pub struct Registry {
    formats: BTreeMap<String, Arc<dyn CustomFormat>>,
}

fn builtin(name: &str) -> Option<Format> {
    serde_json::from_value(serde_json::Value::String(name.into())).ok()
}

impl Registry {
    /// Names must use the x- namespace, with 1–62 lowercase ASCII letters, digits, - or _.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        adapter: impl CustomFormat + 'static,
    ) -> Result<(), Error> {
        let name = name.into();
        if !(3..=64).contains(&name.len())
            || !name.starts_with("x-")
            || !name
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_')
        {
            return Err(Error::new("INVALID_CUSTOM_FORMAT", "Custom names must be 3–64 ASCII characters in the x- namespace, using lowercase letters, digits, - or _."));
        }
        if self.formats.contains_key(&name) {
            return Err(Error::new(
                "DUPLICATE_FORMAT",
                "A custom format is already registered with this name.",
            ));
        }
        self.formats.insert(name, Arc::new(adapter));
        Ok(())
    }

    /// A custom detector never silently overrides another detector or a structured built-in.
    pub fn detect(&self, input: &str) -> Result<String, Error> {
        check_size(input)?;
        let detected = crate::detect(input);
        let matches: Vec<_> = self
            .formats
            .iter()
            .filter(|(_, adapter)| adapter.matches(input))
            .map(|(name, _)| name)
            .collect();
        if matches.len() > 1
            || (!matches.is_empty() && detected.as_ref().is_ok_and(|f| *f != Format::Plain))
        {
            return Err(Error::new(
                "AMBIGUOUS_FORMAT",
                "Multiple formats match; choose a format explicitly.",
            ));
        }
        if let Some(name) = matches.first() {
            return Ok((*name).clone());
        }
        serde_json::to_value(detected?)
            .map_err(|e| Error::new("SERIALIZATION_ERROR", e.to_string()))?
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| Error::new("SERIALIZATION_ERROR", "Expected a built-in format name."))
    }

    pub fn parse(&self, input: &str, format: Option<&str>) -> Result<ParseResult, Error> {
        check_size(input)?;
        let detected;
        let name = match format {
            Some(name) => name,
            None => {
                detected = self.detect(input)?;
                &detected
            }
        };
        if let Some(format) = builtin(name) {
            return crate::parse(input, Some(format));
        }
        let adapter = self.formats.get(name).ok_or_else(|| {
            Error::new(
                "UNKNOWN_FORMAT",
                "Custom format is not registered in this registry.",
            )
        })?;
        validate_custom_parse(adapter.parse(input)?)
    }

    pub fn export(
        &self,
        document: &Document,
        format: &str,
        policy: Policy,
    ) -> Result<ExportResult, Error> {
        validate(document)?;
        if let Some(format) = builtin(format) {
            return crate::export(document, format, policy);
        }
        let adapter = self.formats.get(format).ok_or_else(|| {
            Error::new(
                "UNKNOWN_FORMAT",
                "Custom format is not registered in this registry.",
            )
        })?;
        let result = validate_custom_export(adapter.export(document, policy)?)?;
        let roundtrip = adapter
            .parse(&result.output)
            .ok()
            .map(|parsed| parsed.document);
        verify_custom_export(document, result, roundtrip.as_ref(), policy)
    }
}

/// Validate a host callback result and keep the standard parse-result representation.
pub fn validate_custom_parse(result: ParseResult) -> Result<ParseResult, Error> {
    validate(&result.document)?;
    Ok(result)
}

/// Check a callback's output before asking its parser to verify that output.
pub fn validate_custom_export(result: ExportResult) -> Result<ExportResult, Error> {
    check_size(&result.output)?;
    Ok(result)
}

/// Compare a host-supplied reparse with the original model; callbacks remain host-owned.
pub fn verify_custom_export(
    document: &Document,
    result: ExportResult,
    roundtrip: Option<&Document>,
    policy: Policy,
) -> Result<ExportResult, Error> {
    validate(document)?;
    let mut result = validate_custom_export(result)?;
    // A callback's empty loss list alone cannot prove preservation.
    let valid_roundtrip = roundtrip.filter(|parsed| validate(parsed).is_ok());
    match valid_roundtrip {
        Some(parsed) if parsed == document => {}
        Some(_) => result.losses.push(Diagnostic::new(
            "CUSTOM_ROUNDTRIP_CHANGED",
            "Custom output reparsed into a different canonical document.",
            None,
        )),
        _ => result.losses.push(Diagnostic::new(
            "CUSTOM_ROUNDTRIP_UNVERIFIED",
            "Custom output could not be reparsed into a valid canonical document.",
            None,
        )),
    }
    if policy == Policy::Strict && !result.losses.is_empty() {
        return Err(Error::new(
            "LOSSY_CONVERSION",
            serde_json::to_string(&result.losses)
                .map_err(|e| Error::new("SERIALIZATION_ERROR", e.to_string()))?,
        ));
    }
    Ok(result)
}
