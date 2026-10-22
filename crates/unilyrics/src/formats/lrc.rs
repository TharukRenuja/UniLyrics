use crate::model::*;
use crate::{Error, Format, Policy};

pub fn timestamp(text: &str) -> Option<u64> {
    let (minutes, tail) = text.split_once(':')?;
    if minutes.is_empty() || !minutes.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (seconds, fraction) = tail.split_once('.').unwrap_or((tail, ""));
    if seconds.len() != 2 || !seconds.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if fraction.len() > 6 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if tail.ends_with('.') {
        return None;
    }
    let seconds: u64 = seconds.parse().ok()?;
    if seconds >= 60 {
        return None;
    }
    let fraction_us = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse::<u64>()
            .ok()?
            .checked_mul(10_u64.pow(6 - fraction.len() as u32))?
    };
    minutes
        .parse::<u64>()
        .ok()?
        .checked_mul(60)?
        .checked_add(seconds)?
        .checked_mul(1_000_000)?
        .checked_add(fraction_us)
}

pub fn stamp(time: u64) -> String {
    let seconds = time / 1_000_000;
    format!(
        "{:02}:{:02}.{:06}",
        seconds / 60,
        seconds % 60,
        time % 1_000_000
    )
}

fn segments(text: &str) -> Vec<Segment> {
    let mut result: Vec<Segment> = vec![];
    let mut rest = text;
    let mut pending = None;
    let mut buffer = String::new();
    while let Some(open) = rest.find('<') {
        let Some(close_rel) = rest[open + 1..].find('>') else {
            break;
        };
        let close = open + 1 + close_rel;
        let Some(time) = timestamp(&rest[open + 1..close]) else {
            buffer.push_str(&rest[..close + 1]);
            rest = &rest[close + 1..];
            continue;
        };
        buffer.push_str(&rest[..open]);
        if !buffer.is_empty() {
            result.push(Segment {
                text: std::mem::take(&mut buffer),
                start_us: pending,
                end_us: None,
                attributes: Attributes::new(),
                ..Segment::default()
            });
        } else if let Some(previous) = result.last_mut() {
            previous.end_us = pending;
        } else if pending.is_some() {
            result.push(Segment {
                text: String::new(),
                start_us: pending,
                end_us: Some(time),
                attributes: Attributes::new(),
                ..Segment::default()
            });
        }
        pending = Some(time);
        rest = &rest[close + 1..];
    }
    buffer.push_str(rest);
    if !buffer.is_empty() {
        result.push(Segment {
            text: buffer,
            start_us: pending,
            end_us: None,
            attributes: Attributes::new(),
            ..Segment::default()
        });
    } else if let Some(last) = result.last_mut() {
        last.end_us = pending;
    } else if pending.is_some() {
        result.push(Segment {
            text: String::new(),
            start_us: pending,
            end_us: None,
            attributes: Attributes::new(),
            ..Segment::default()
        });
    }
    result
}

pub fn parse(input: &str, enhanced: bool) -> Result<ParseResult, Error> {
    let mut document = Document::default();
    let mut diagnostics = vec![];
    for raw in input.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let mut rest = line;
        let mut times = vec![];
        while let Some(tag) = rest.strip_prefix('[') {
            let Some(end) = tag.find(']') else {
                break;
            };
            let Some(time) = timestamp(&tag[..end]) else {
                break;
            };
            times.push(Some(time));
            rest = &tag[end + 1..];
        }
        if times.is_empty() {
            if let Some(tag) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                if let Some((key, value)) = tag.split_once(':') {
                    if !key.is_empty() && key.bytes().all(|c| c.is_ascii_alphabetic()) {
                        document.metadata.push(Metadata {
                            key: key.into(),
                            value: value.into(),
                        });
                        if key.eq_ignore_ascii_case("offset") {
                            diagnostics.push(Diagnostic::new("OFFSET_NOT_APPLIED", "Raw offset retained; timestamps are not shifted without a defined dialect.", None));
                        }
                        continue;
                    }
                }
            }
            if line.starts_with('[') && line.as_bytes().get(1).is_some_and(u8::is_ascii_digit) {
                return Err(Error::new(
                    "INVALID_TIMESTAMP",
                    format!("Invalid LRC timestamp: {line}"),
                ));
            }
            if !line.is_empty() {
                diagnostics.push(Diagnostic::new(
                    "UNTIMED_CONTENT",
                    "Untimed content retained in an LRC document.",
                    None,
                ));
            }
            times.push(None);
            rest = line;
        }
        let repeated = times.len() > 1;
        for start in times {
            let parts = if enhanced {
                segments(rest)
            } else {
                vec![Segment {
                    text: rest.into(),
                    start_us: None,
                    end_us: None,
                    attributes: Attributes::new(),
                    ..Segment::default()
                }]
            };
            if repeated && enhanced && parts.iter().any(|s| s.start_us.is_some()) {
                diagnostics.push(Diagnostic::new("REPEATED_SEGMENT_TIMES", "Repeated line timestamps retain absolute segment times; no timing shift was invented.", None));
            }
            document.cues.push(Cue {
                id: format!("L{}", document.cues.len() + 1),
                start_us: start,
                end_us: None,
                segments: parts,
                attributes: Attributes::new(),
                ..Cue::default()
            });
        }
    }
    Ok(ParseResult {
        document,
        diagnostics,
    })
}

pub fn export(document: &Document, format: Format, policy: Policy) -> Result<ExportResult, Error> {
    let mut losses = vec![];
    let mut lines = vec![];
    if document.cues.is_empty() && (format == Format::Plain || document.metadata.is_empty()) {
        losses.push(Diagnostic::new(
            "CUE_BOUNDARIES_CHANGED",
            "Target parses empty output as an empty cue rather than no cues.",
            None,
        ));
    }
    for (present, code, message) in [
        (
            !document.agents.is_empty(),
            "AGENTS_DROPPED",
            "Target cannot preserve the vocal-agent registry.",
        ),
        (
            !document.auxiliary.is_empty(),
            "AUXILIARY_DROPPED",
            "Target cannot preserve auxiliary text and its alignment relationships.",
        ),
        (
            document.language.is_some(),
            "LANGUAGE_DROPPED",
            "Target cannot preserve the document language.",
        ),
    ] {
        if present {
            losses.push(Diagnostic::new(code, message, None));
        }
    }
    if format != Format::Plain {
        for meta in &document.metadata {
            if meta.key.is_empty()
                || !meta.key.bytes().all(|c| c.is_ascii_alphabetic())
                || meta.value.contains([']', '\n', '\r'])
            {
                return Err(Error::new(
                    "UNREPRESENTABLE_METADATA",
                    "LRC metadata contains reserved characters.",
                ));
            }
            lines.push(format!("[{}:{}]", meta.key, meta.value));
        }
    } else if !document.metadata.is_empty() {
        losses.push(Diagnostic::new(
            "METADATA_DROPPED",
            "Plain text cannot preserve structured metadata.",
            None,
        ));
    }
    if !document.attributes.is_empty() {
        losses.push(Diagnostic::new(
            "ATTRIBUTES_DROPPED",
            "Target cannot preserve document attributes.",
            None,
        ));
    }
    for (cue_index, cue) in document.cues.iter().enumerate() {
        if cue.id != format!("L{}", cue_index + 1) {
            losses.push(Diagnostic::new(
                "CUE_IDS_CHANGED",
                "Target regenerates cue identities from line order.",
                Some(&cue.id),
            ));
        }
        for (present, code, message) in [
            (
                cue.language.is_some()
                    || cue
                        .segments
                        .iter()
                        .any(|segment| segment.language.is_some()),
                "LANGUAGE_DROPPED",
                "Target cannot preserve cue/segment languages.",
            ),
            (
                !cue.agent_ids.is_empty()
                    || cue
                        .segments
                        .iter()
                        .any(|segment| !segment.agent_ids.is_empty()),
                "VOCAL_AGENTS_DROPPED",
                "Target cannot preserve vocal-agent references.",
            ),
            (
                !cue.roles.is_empty()
                    || cue.segments.iter().any(|segment| !segment.roles.is_empty()),
                "ROLES_DROPPED",
                "Target cannot preserve cue/segment roles.",
            ),
            (
                cue.parent_id.is_some(),
                "RELATIONSHIPS_DROPPED",
                "Target cannot preserve the cue parent relationship.",
            ),
            (
                cue.segments.iter().any(|segment| segment.id.is_some()),
                "SEGMENT_IDS_DROPPED",
                "Target cannot preserve segment identities.",
            ),
        ] {
            if present {
                losses.push(Diagnostic::new(code, message, Some(&cue.id)));
            }
        }
        let mut line = String::new();
        let text = cue.text();
        if format != Format::Plain && text.starts_with('[') {
            return Err(Error::new(
                "UNREPRESENTABLE_TEXT",
                "Leading brackets can be reinterpreted as LRC timestamps or metadata.",
            ));
        }
        if format == Format::EnhancedLrc
            && text.split('<').skip(1).any(|part| {
                part.split_once('>')
                    .and_then(|(time, _)| timestamp(time))
                    .is_some()
            })
        {
            return Err(Error::new("UNREPRESENTABLE_TEXT", "Literal text contains an Enhanced LRC timestamp; this profile has no escape syntax."));
        }
        if format != Format::Plain {
            if let Some(start) = cue.start_us {
                line.push_str(&format!("[{}]", stamp(start)));
            }
        } else if cue.start_us.is_some()
            || cue.end_us.is_some()
            || cue
                .segments
                .iter()
                .any(|s| s.start_us.is_some() || s.end_us.is_some())
        {
            losses.push(Diagnostic::new(
                "TIMING_DROPPED",
                "Plain text drops timing.",
                Some(&cue.id),
            ));
        }
        if cue.end_us.is_some() && format != Format::Plain {
            losses.push(Diagnostic::new(
                "CUE_END_DROPPED",
                "This LRC profile does not encode cue ends.",
                Some(&cue.id),
            ));
        }
        if !cue.attributes.is_empty() || cue.segments.iter().any(|s| !s.attributes.is_empty()) {
            losses.push(Diagnostic::new(
                "ATTRIBUTES_DROPPED",
                "Target cannot preserve cue/segment attributes.",
                Some(&cue.id),
            ));
        }
        for segment in &cue.segments {
            if format != Format::Plain && segment.text.contains(['\n', '\r']) {
                return Err(Error::new(
                    "UNREPRESENTABLE_TEXT",
                    "A cue with embedded newlines cannot be exported by this profile.",
                ));
            }
            if format == Format::EnhancedLrc {
                if let Some(start) = segment.start_us {
                    line.push_str(&format!("<{}>", stamp(start)));
                }
                line.push_str(&segment.text);
                if let Some(end) = segment.end_us {
                    line.push_str(&format!("<{}>", stamp(end)));
                }
            } else {
                line.push_str(&segment.text);
            }
        }
        if format == Format::Lrc
            && cue
                .segments
                .iter()
                .any(|s| s.start_us.is_some() || s.end_us.is_some())
        {
            losses.push(Diagnostic::new(
                "SEGMENT_TIMING_DROPPED",
                "Baseline LRC drops subline timing.",
                Some(&cue.id),
            ));
        }
        if format == Format::EnhancedLrc {
            let projected = parse(&line, true)?;
            let parts = |cue: &Cue| {
                cue.segments
                    .iter()
                    .map(|s| (s.text.clone(), s.start_us, s.end_us))
                    .collect::<Vec<_>>()
            };
            if projected.document.cues.len() != 1
                || parts(&projected.document.cues[0]) != parts(cue)
            {
                losses.push(Diagnostic::new("SEGMENT_STRUCTURE_CHANGED", "Enhanced LRC cannot preserve this combination of untimed segments and timing boundaries.", Some(&cue.id)));
            }
        } else if cue.segments.is_empty() {
            losses.push(Diagnostic::new(
                "SEGMENT_STRUCTURE_CHANGED",
                "Target creates an empty text segment for a cue without segments.",
                Some(&cue.id),
            ));
        } else if cue.segments.len() > 1
            && cue
                .segments
                .iter()
                .all(|s| s.start_us.is_none() && s.end_us.is_none())
        {
            losses.push(Diagnostic::new(
                "SEGMENT_BOUNDARIES_DROPPED",
                "Target cannot preserve separate untimed text runs.",
                Some(&cue.id),
            ));
        }
        if format == Format::Plain && cue.segments.iter().any(|s| s.text.contains('\n')) {
            losses.push(Diagnostic::new(
                "CUE_BOUNDARIES_CHANGED",
                "An embedded newline becomes a separate plain-text line.",
                Some(&cue.id),
            ));
        }
        lines.push(line);
    }
    if policy == Policy::Strict && !losses.is_empty() {
        return Err(Error::new(
            "LOSSY_CONVERSION",
            serde_json::to_string(&losses).unwrap_or_default(),
        ));
    }
    Ok(ExportResult {
        output: lines.join("\n"),
        losses,
    })
}
