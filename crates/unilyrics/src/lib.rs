//! Offline lyrics parsing and conversion. Public schema is experimental until 1.0.
mod extensions;
pub use extensions::{
    validate_custom_export, validate_custom_parse, verify_custom_export, CustomFormat, Registry,
};
mod features;
mod model;
mod normalization;
mod preservation;
mod profiles;
pub use normalization::normalize;
pub use preservation::{
    export_preserved, restore_preserved, PreservationSidecar, PreservedExportResult,
};
pub use profiles::{export_profile, project, Profile, ProjectionFeature, ProjectionOptions};
mod validation;
pub use features::inspect;
pub use validation::validate;
mod formats {
    pub mod lrc;
    pub mod srt;
    pub mod ttml;
    pub mod webvtt;
}

pub use model::*;
use serde::{Deserialize, Serialize};

pub const MAX_INPUT_BYTES: usize = 1_048_576;
pub const MAX_SAFE_TIME_US: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Plain,
    Lrc,
    EnhancedLrc,
    Json,
    Ttml,
    TtmlLyrics,
    Srt,
    Webvtt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Policy {
    Strict,
    #[default]
    Compatible,
}

pub fn detect(input: &str) -> Result<Format, Error> {
    check_size(input)?;
    let trimmed = input.trim_start_matches('\u{feff}').trim_start();
    if trimmed
        .strip_prefix("WEBVTT")
        .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with([' ', '\t', '\r', '\n']))
    {
        return Ok(Format::Webvtt);
    }
    if formats::srt::looks_like(trimmed) {
        return Ok(Format::Srt);
    }
    if trimmed.starts_with('{') {
        serde_json::from_str::<serde_json::Value>(trimmed)
            .map_err(|e| Error::new("INVALID_JSON", e.to_string()))?;
        return Ok(Format::Json);
    }
    if trimmed.starts_with('[') && serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
        return Ok(Format::Json);
    }
    if trimmed.starts_with('<') && !trimmed.starts_with("<00:") {
        formats::ttml::detect(trimmed)?;
        return Ok(Format::Ttml);
    }
    let line_timed = input.lines().any(|line| {
        line.strip_prefix('[')
            .and_then(|s| s.split_once(']'))
            .and_then(|(s, _)| formats::lrc::timestamp(s))
            .is_some()
    });
    if line_timed {
        let segment_timed = input.split('<').skip(1).any(|part| {
            part.split_once('>')
                .and_then(|(time, _)| formats::lrc::timestamp(time))
                .is_some()
        });
        Ok(if segment_timed {
            Format::EnhancedLrc
        } else {
            Format::Lrc
        })
    } else if trimmed.starts_with('[') && trimmed.as_bytes().get(1).is_some_and(u8::is_ascii_digit)
    {
        Err(Error::new(
            "INVALID_TIMESTAMP",
            "Malformed timestamp; specify plain explicitly to keep literal text.",
        ))
    } else if input.lines().any(|line| {
        line.strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .and_then(|s| s.split_once(':'))
            .is_some_and(|(key, _)| !key.is_empty() && key.bytes().all(|c| c.is_ascii_alphabetic()))
    }) {
        Ok(Format::Lrc)
    } else {
        Ok(Format::Plain)
    }
}

fn check_size(input: &str) -> Result<(), Error> {
    if input.len() > MAX_INPUT_BYTES {
        Err(Error::new("INPUT_TOO_LARGE", "Input exceeds 1 MiB."))
    } else {
        Ok(())
    }
}

pub fn parse(input: &str, format: Option<Format>) -> Result<ParseResult, Error> {
    check_size(input)?;
    let format = match format {
        Some(f) => f,
        None => detect(input)?,
    };
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let result = match format {
        Format::Webvtt => formats::webvtt::parse(input)?,
        Format::Srt => formats::srt::parse(input)?,
        Format::Ttml | Format::TtmlLyrics => {
            formats::ttml::parse(input, format == Format::TtmlLyrics)?
        }
        Format::Json => ParseResult {
            document: serde_json::from_str(input)
                .map_err(|e| Error::new("INVALID_JSON", e.to_string()))?,
            diagnostics: vec![],
        },
        Format::Lrc | Format::EnhancedLrc => {
            formats::lrc::parse(input, format == Format::EnhancedLrc)?
        }
        Format::Plain => ParseResult {
            document: Document {
                cues: input
                    .split('\n')
                    .enumerate()
                    .map(|(i, text)| Cue {
                        id: format!("L{}", i + 1),
                        start_us: None,
                        end_us: None,
                        segments: vec![Segment {
                            text: text.into(),
                            start_us: None,
                            end_us: None,
                            attributes: Attributes::new(),
                            ..Segment::default()
                        }],
                        attributes: Attributes::new(),
                        ..Cue::default()
                    })
                    .collect(),
                ..Document::default()
            },
            diagnostics: vec![],
        },
    };
    validate(&result.document)?;
    Ok(result)
}

pub fn export(document: &Document, format: Format, policy: Policy) -> Result<ExportResult, Error> {
    validate(document)?;
    if format == Format::Webvtt {
        return formats::webvtt::export(document, policy);
    }
    if format == Format::Srt {
        return formats::srt::export(document, policy);
    }
    if matches!(format, Format::Ttml | Format::TtmlLyrics) {
        return formats::ttml::export(document, format == Format::TtmlLyrics, policy);
    }
    if format == Format::Json {
        Ok(ExportResult {
            output: serde_json::to_string(document)
                .map_err(|e| Error::new("SERIALIZATION_ERROR", e.to_string()))?,
            losses: vec![],
        })
    } else {
        formats::lrc::export(document, format, policy)
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Parse {
        input: String,
        format: Option<Format>,
    },
    Detect {
        input: String,
    },
    Export {
        document: Document,
        format: Format,
        #[serde(default)]
        policy: Policy,
    },
    Validate {
        document: Document,
    },
    Inspect {
        document: Document,
    },
    Normalize {
        document: Document,
        #[serde(default)]
        offset_us: i64,
    },
    Project {
        document: Document,
        #[serde(default)]
        options: ProjectionOptions,
    },
    ExportProfile {
        document: Document,
        format: Format,
        #[serde(default)]
        options: ProjectionOptions,
        #[serde(default)]
        policy: Policy,
    },
    ExportPreserved {
        document: Document,
        format: Format,
        #[serde(default)]
        options: ProjectionOptions,
    },
    RestorePreserved {
        input: String,
        format: Format,
        sidecar: PreservationSidecar,
    },
    ValidateCustomParse {
        result: ParseResult,
    },
    ValidateCustomExport {
        result: ExportResult,
    },
    VerifyCustomExport {
        document: Document,
        result: ExportResult,
        #[serde(default)]
        roundtrip: Option<Document>,
        #[serde(default)]
        policy: Policy,
    },
}

/// Shared JSON protocol for language bindings; errors are JSON, never console output.
pub fn process(request: &str) -> String {
    fn run(request: &str) -> Result<serde_json::Value, Error> {
        check_size(request)?;
        let request: Request = serde_json::from_str(request)
            .map_err(|e| Error::new("INVALID_REQUEST", e.to_string()))?;
        match request {
            Request::Parse { input, format } => serde_json::to_value(parse(&input, format)?),
            Request::Detect { input } => serde_json::to_value(detect(&input)?),
            Request::Export {
                document,
                format,
                policy,
            } => serde_json::to_value(export(&document, format, policy)?),
            Request::Inspect { document } => serde_json::to_value(inspect(&document)?),
            Request::Project { document, options } => {
                serde_json::to_value(project(&document, &options)?)
            }
            Request::ExportProfile {
                document,
                format,
                options,
                policy,
            } => serde_json::to_value(export_profile(&document, format, &options, policy)?),
            Request::ExportPreserved {
                document,
                format,
                options,
            } => serde_json::to_value(export_preserved(&document, format, &options)?),
            Request::RestorePreserved {
                input,
                format,
                sidecar,
            } => serde_json::to_value(restore_preserved(&input, format, &sidecar)?),
            Request::ValidateCustomParse { result } => {
                serde_json::to_value(validate_custom_parse(result)?)
            }
            Request::ValidateCustomExport { result } => {
                serde_json::to_value(validate_custom_export(result)?)
            }
            Request::VerifyCustomExport {
                document,
                result,
                roundtrip,
                policy,
            } => serde_json::to_value(verify_custom_export(
                &document,
                result,
                roundtrip.as_ref(),
                policy,
            )?),
            Request::Normalize {
                document,
                offset_us,
            } => serde_json::to_value(normalize(&document, offset_us)?),
            Request::Validate { document } => {
                validate(&document)?;
                Ok(serde_json::json!({"valid":true}))
            }
        }
        .map_err(|e| Error::new("SERIALIZATION_ERROR", e.to_string()))
    }
    match run(request) {
        Ok(result) => serde_json::json!({"ok":true,"result":result}).to_string(),
        Err(error) => serde_json::json!({"ok":false,"error":error}).to_string(),
    }
}
