use crate::{
    check_size, export_profile, validate, Diagnostic, Document, Error, Format, ParseResult, Policy,
    ProjectionOptions,
};
use serde::{Deserialize, Serialize};

/// Canonical snapshot bound to exact exported UTF-8 content, not source-file bytes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreservationSidecar {
    pub version: u32,
    pub format: Format,
    pub output: String,
    pub document: Document,
}

/// Preserve requires keeping both output and sidecar; losses still describe output alone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreservedExportResult {
    pub output: String,
    pub losses: Vec<Diagnostic>,
    pub sidecar: PreservationSidecar,
}

fn check_restore_size(
    output: &str,
    format: Format,
    sidecar: &PreservationSidecar,
) -> Result<(), Error> {
    // Keep every generated bundle usable through the same bounded shared protocol.
    let request = serde_json::json!({"op":"restore_preserved", "input":output, "format":format, "sidecar":sidecar});
    check_size(&request.to_string())
}

/// Compatible target output plus a full original snapshot, including profile removals.
pub fn export_preserved(
    document: &Document,
    format: Format,
    options: &ProjectionOptions,
) -> Result<PreservedExportResult, Error> {
    let result = export_profile(document, format, options, Policy::Compatible)?;
    let sidecar = PreservationSidecar {
        version: 1,
        format,
        output: result.output.clone(),
        document: document.clone(),
    };
    check_restore_size(&result.output, format, &sidecar)?;
    Ok(PreservedExportResult {
        output: result.output,
        losses: result.losses,
        sidecar,
    })
}

/// Restore a validated snapshot only when the accompanying output/format match exactly.
/// Sidecars are untrusted metadata, not authenticated provenance or edit-merge instructions.
pub fn restore_preserved(
    output: &str,
    format: Format,
    sidecar: &PreservationSidecar,
) -> Result<ParseResult, Error> {
    check_size(output)?;
    check_restore_size(output, format, sidecar)?;
    if sidecar.version != 1 {
        return Err(Error::new(
            "UNSUPPORTED_SIDECAR_VERSION",
            "Unsupported preservation sidecar version.",
        ));
    }
    if sidecar.format != format || sidecar.output != output {
        return Err(Error::new(
            "SIDECAR_MISMATCH",
            "Sidecar does not match the exact output and format.",
        ));
    }
    validate(&sidecar.document)?;
    Ok(ParseResult {
        document: sidecar.document.clone(),
        diagnostics: vec![Diagnostic::new(
            "SIDECAR_RESTORED",
            "Original canonical document restored from its matching sidecar.",
            None,
        )],
    })
}
