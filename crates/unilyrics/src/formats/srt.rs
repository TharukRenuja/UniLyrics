use crate::{
    Attributes, Cue, Diagnostic, Document, Error, ExportResult, ParseResult, Policy, Segment,
    MAX_SAFE_TIME_US,
};

fn invalid() -> Error {
    Error::new(
        "INVALID_SRT",
        "Expected a numeric index, HH:MM:SS,mmm --> HH:MM:SS,mmm, and cue text.",
    )
}

fn index(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|c| c.is_ascii_digit())
}

fn time(value: &str) -> Result<u64, Error> {
    let (clock, millis) = value.split_once(',').ok_or_else(invalid)?;
    let parts: Vec<_> = clock.split(':').collect();
    let [hours, minutes, seconds] = parts.as_slice() else {
        return Err(invalid());
    };
    if hours.len() < 2
        || minutes.len() != 2
        || seconds.len() != 2
        || millis.len() != 3
        || !parts.iter().all(|part| index(part))
        || !index(millis)
    {
        return Err(invalid());
    }
    let hours: u64 = hours.parse().map_err(|_| invalid())?;
    let minutes: u64 = minutes.parse().map_err(|_| invalid())?;
    let seconds: u64 = seconds.parse().map_err(|_| invalid())?;
    let millis: u64 = millis.parse().map_err(|_| invalid())?;
    if minutes >= 60 || seconds >= 60 {
        return Err(invalid());
    }
    hours
        .checked_mul(3600)
        .and_then(|n| n.checked_add(minutes * 60 + seconds))
        .and_then(|n| n.checked_mul(1_000_000))
        .and_then(|n| n.checked_add(millis * 1000))
        .filter(|n| *n <= MAX_SAFE_TIME_US)
        .ok_or_else(invalid)
}

pub fn looks_like(input: &str) -> bool {
    let mut lines = input.split(['\r', '\n']).filter(|line| !line.is_empty());
    lines.next().is_some_and(|line| index(line.trim()))
        && lines.next().is_some_and(|line| line.contains("-->"))
}

pub fn parse(input: &str) -> Result<ParseResult, Error> {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = normalized.split('\n').peekable();
    let mut document = Document::default();
    let mut diagnostics = vec![];
    if input.contains('\r') {
        diagnostics.push(Diagnostic::new(
            "NEWLINES_NORMALIZED",
            "SRT line separators were normalized to LF.",
            None,
        ));
    }
    while let Some(counter) = lines.next() {
        if counter.is_empty() {
            continue;
        }
        let counter = counter.trim();
        if !index(counter) {
            return Err(invalid());
        }
        let timing = lines.next().ok_or_else(invalid)?;
        let (start, end) = timing.split_once("-->").ok_or_else(invalid)?;
        let start = time(start.trim())?;
        let end = time(end.trim())?;
        let mut payload = vec![];
        while lines.peek().is_some_and(|line| !line.is_empty()) {
            payload.push(lines.next().unwrap());
        }
        let text = payload.join("\n");
        let number = document.cues.len() + 1;
        let id = format!("L{number}");
        let mut attributes = Attributes::new();
        if counter != number.to_string() {
            attributes.insert("srt:index".into(), counter.into());
        }
        if text.contains(['<', '&']) || text.contains("{\\") {
            diagnostics.push(Diagnostic::new(
                "SRT_MARKUP_UNINTERPRETED",
                "Subtitle markup is retained verbatim; its styling semantics are not interpreted.",
                Some(&id),
            ));
        }
        document.cues.push(Cue {
            id,
            start_us: Some(start),
            end_us: Some(end),
            attributes,
            segments: vec![Segment {
                text,
                ..Segment::default()
            }],
            ..Cue::default()
        });
        if document.cues.len() > 10_000 {
            return Err(Error::new("TOO_MANY_CUES", "Document exceeds 10000 cues."));
        }
    }
    Ok(ParseResult {
        document,
        diagnostics,
    })
}

fn stamp(time: u64) -> String {
    let seconds = time / 1_000_000;
    format!(
        "{:02}:{:02}:{:02},{:03}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60,
        time / 1000 % 1000
    )
}

pub fn export(document: &Document, policy: Policy) -> Result<ExportResult, Error> {
    let mut output = String::new();
    let mut losses = vec![];
    for (present, code, message) in [
        (
            !document.metadata.is_empty(),
            "METADATA_DROPPED",
            "SRT cannot encode document metadata.",
        ),
        (
            !document.agents.is_empty(),
            "AGENTS_DROPPED",
            "SRT cannot encode the vocal-agent registry.",
        ),
        (
            !document.auxiliary.is_empty(),
            "AUXILIARY_DROPPED",
            "SRT cannot encode linked auxiliary tracks.",
        ),
        (
            document.language.is_some(),
            "LANGUAGE_DROPPED",
            "SRT cannot encode the document language.",
        ),
        (
            !document.attributes.is_empty(),
            "ATTRIBUTES_DROPPED",
            "SRT cannot encode document attributes.",
        ),
    ] {
        if present {
            losses.push(Diagnostic::new(code, message, None));
        }
    }
    for (position, cue) in document.cues.iter().enumerate() {
        let (Some(start), Some(end)) = (cue.start_us, cue.end_us) else {
            return Err(Error::new(
                "MISSING_TIMING",
                "SRT requires explicit cue starts and ends; missing ends are not estimated.",
            ));
        };
        let generated = (position + 1).to_string();
        let counter = match cue.attributes.get("srt:index") {
            None => &generated,
            Some(value) => value.as_str().filter(|value| index(value)).ok_or_else(|| {
                Error::new(
                    "UNREPRESENTABLE_METADATA",
                    "srt:index must be a nonempty decimal string.",
                )
            })?,
        };
        let text = cue.text();
        if text.contains('\r')
            || text.starts_with('\n')
            || text.ends_with('\n')
            || text.contains("\n\n")
        {
            return Err(Error::new(
                "UNREPRESENTABLE_TEXT",
                "SRT cannot preserve embedded blank lines or carriage returns in a cue.",
            ));
        }
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&format!(
            "{counter}\n{} --> {}\n{text}\n",
            stamp(start),
            stamp(end)
        ));
        for (present, code, message) in [
            (
                start % 1000 != 0 || end % 1000 != 0,
                "TIME_PRECISION_REDUCED",
                "SRT floors cue times to milliseconds.",
            ),
            (
                cue.id != format!("L{}", position + 1),
                "CUE_IDS_CHANGED",
                "SRT regenerates cue identities from source order.",
            ),
            (
                cue.language.is_some() || cue.segments.iter().any(|s| s.language.is_some()),
                "LANGUAGE_DROPPED",
                "SRT cannot encode cue/segment languages.",
            ),
            (
                !cue.agent_ids.is_empty() || cue.segments.iter().any(|s| !s.agent_ids.is_empty()),
                "VOCAL_AGENTS_DROPPED",
                "SRT cannot encode vocal-agent references.",
            ),
            (
                !cue.roles.is_empty() || cue.segments.iter().any(|s| !s.roles.is_empty()),
                "ROLES_DROPPED",
                "SRT cannot encode cue/segment roles.",
            ),
            (
                cue.parent_id.is_some(),
                "RELATIONSHIPS_DROPPED",
                "SRT cannot encode cue parents.",
            ),
            (
                cue.segments.iter().any(|s| s.id.is_some()),
                "SEGMENT_IDS_DROPPED",
                "SRT cannot encode segment identities.",
            ),
            (
                cue.segments
                    .iter()
                    .any(|s| s.start_us.is_some() || s.end_us.is_some()),
                "SEGMENT_TIMING_DROPPED",
                "SRT cannot encode segment timing.",
            ),
            (
                cue.segments.len() != 1,
                "SEGMENT_BOUNDARIES_DROPPED",
                "SRT flattens separate segments into one cue payload.",
            ),
            (
                cue.attributes.keys().any(|key| key != "srt:index")
                    || cue.segments.iter().any(|s| !s.attributes.is_empty()),
                "ATTRIBUTES_DROPPED",
                "SRT cannot encode these cue/segment attributes.",
            ),
        ] {
            if present {
                losses.push(Diagnostic::new(code, message, Some(&cue.id)));
            }
        }
    }
    if parse(&output)?.document != *document {
        losses.push(Diagnostic::new(
            "SRT_PROJECTION_CHANGED",
            "The reparsed SRT projection differs from the canonical document.",
            None,
        ));
    }
    if policy == Policy::Strict && !losses.is_empty() {
        return Err(Error::new(
            "LOSSY_CONVERSION",
            serde_json::to_string(&losses).unwrap_or_default(),
        ));
    }
    Ok(ExportResult { output, losses })
}
