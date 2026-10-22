use crate::{
    Agent, Attributes, Cue, Diagnostic, Document, Error, ExportResult, ParseResult, Policy,
    Segment, MAX_SAFE_TIME_US,
};
use serde::{Deserialize, Serialize};

fn invalid(message: &str) -> Error {
    Error::new("INVALID_WEBVTT", message)
}
fn unsupported(message: &str) -> Error {
    Error::new("UNSUPPORTED_WEBVTT", message)
}

fn time(value: &str) -> Result<u64, Error> {
    let (clock, millis) = value
        .split_once('.')
        .ok_or_else(|| invalid("Expected [HH:]MM:SS.mmm."))?;
    let parts: Vec<_> = clock.split(':').collect();
    let (hours, minutes, seconds) = match parts.as_slice() {
        [minutes, seconds] => ("00", *minutes, *seconds),
        [hours, minutes, seconds] => (*hours, *minutes, *seconds),
        _ => return Err(invalid("Invalid WebVTT timestamp.")),
    };
    if hours.len() < 2
        || minutes.len() != 2
        || seconds.len() != 2
        || millis.len() != 3
        || ![hours, minutes, seconds, millis]
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit()))
    {
        return Err(invalid("Expected [HH:]MM:SS.mmm."));
    }
    let values: Vec<u64> = [hours, minutes, seconds, millis]
        .iter()
        .map(|part| part.parse().map_err(|_| invalid("Timestamp overflow.")))
        .collect::<Result<_, _>>()?;
    if values[1] >= 60 || values[2] >= 60 {
        return Err(invalid("Minutes and seconds must be below 60."));
    }
    values[0]
        .checked_mul(3600)
        .and_then(|n| n.checked_add(values[1] * 60 + values[2]))
        .and_then(|n| n.checked_mul(1_000_000))
        .and_then(|n| n.checked_add(values[3] * 1000))
        .filter(|n| *n <= MAX_SAFE_TIME_US)
        .ok_or_else(|| invalid("Timestamp outside the supported range."))
}

fn decoded(value: &str) -> Result<String, Error> {
    let mut result = String::new();
    let mut rest = value;
    while let Some(position) = rest.find('&') {
        result.push_str(&rest[..position]);
        rest = &rest[position + 1..];
        let (name, tail) = rest.split_once(';').ok_or_else(|| {
            unsupported("Character references without semicolons are not implemented.")
        })?;
        let character = match name {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "nbsp" => '\u{a0}',
            "lrm" => '\u{200e}',
            "rlm" => '\u{200f}',
            _ => return Err(unsupported(
                "This WebVTT profile accepts amp, lt, gt, nbsp, lrm and rlm character references.",
            )),
        };
        result.push(character);
        rest = tail;
    }
    result.push_str(rest);
    Ok(result)
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tag {
    group: u64,
    tag: String,
}

fn tag_name(raw: &str) -> &str {
    raw.split(['.', ' ', '\t']).next().unwrap_or("")
}
fn annotation(raw: &str) -> Option<&str> {
    raw.split_once([' ', '\t'])
        .map(|(_, annotation)| annotation.trim())
}

fn check_tag(raw: &str) -> Result<(), Error> {
    if raw.contains(['<', '>', '\n', '\r']) {
        return Err(invalid("Invalid span tag."));
    }
    let name = tag_name(raw);
    if !matches!(name, "c" | "b" | "i" | "u" | "ruby" | "rt" | "v" | "lang") {
        return Err(unsupported("Unknown WebVTT span tag."));
    }
    let token = raw.split([' ', '\t']).next().unwrap_or("");
    if token.split('.').skip(1).any(str::is_empty) {
        return Err(invalid("Empty span class."));
    }
    match annotation(raw) {
        Some(value) if matches!(name, "v" | "lang") && !value.is_empty() => {
            if name == "lang" && value.split_whitespace().count() != 1 {
                return Err(invalid("A language annotation must be one nonempty tag."));
            }
        }
        None if !matches!(name, "v" | "lang") => {}
        _ => return Err(invalid("Invalid or missing span annotation.")),
    }
    Ok(())
}

fn emit(
    buffer: &mut String,
    segments: &mut Vec<Segment>,
    stack: &[Tag],
    pending: Option<u64>,
    agents: &mut Vec<Agent>,
    force: bool,
) -> Result<(), Error> {
    if buffer.is_empty() && !force {
        return Ok(());
    }
    let mut segment = Segment {
        text: decoded(buffer)?,
        start_us: pending,
        ..Segment::default()
    };
    buffer.clear();
    if !stack.is_empty() {
        segment
            .attributes
            .insert("webvtt:tags".into(), serde_json::to_value(stack).unwrap());
    }
    if let Some(tag) = stack.iter().rev().find(|tag| tag_name(&tag.tag) == "lang") {
        segment.language = annotation(&tag.tag).map(str::to_owned);
    }
    if let Some(tag) = stack.iter().rev().find(|tag| tag_name(&tag.tag) == "v") {
        let name = annotation(&tag.tag).unwrap();
        let id = if let Some(agent) = agents
            .iter()
            .find(|agent| agent.name.as_deref() == Some(name))
        {
            agent.id.clone()
        } else {
            if agents.len() >= 10_000 {
                return Err(Error::new(
                    "TOO_MANY_AGENTS",
                    "Document exceeds 10000 vocal agents.",
                ));
            }
            let id = format!("V{}", agents.len() + 1);
            agents.push(Agent {
                id: id.clone(),
                name: Some(name.into()),
                kind: Some("voice".into()),
                attributes: Attributes::new(),
            });
            id
        };
        segment.agent_ids.push(id);
    }
    if stack.iter().any(|tag| tag_name(&tag.tag) == "rt") {
        segment.roles.push("ruby_text".into());
    }
    segments.push(segment);
    Ok(())
}

fn payload(
    text: &str,
    start: u64,
    end: u64,
    agents: &mut Vec<Agent>,
) -> Result<Vec<Segment>, Error> {
    let mut segments = vec![];
    let mut stack: Vec<Tag> = vec![];
    let mut buffer = String::new();
    let mut pending = None;
    let mut group = 0;
    let mut rest = text;
    while let Some(position) = rest.find('<') {
        buffer.push_str(&rest[..position]);
        let (raw, tail) = rest[position + 1..]
            .split_once('>')
            .ok_or_else(|| invalid("Unterminated span or timestamp."))?;
        emit(&mut buffer, &mut segments, &stack, pending, agents, false)?;
        if raw.as_bytes().first().is_some_and(u8::is_ascii_digit) {
            if pending.is_some()
                && segments
                    .last()
                    .is_none_or(|segment| segment.start_us != pending)
            {
                emit(&mut buffer, &mut segments, &stack, pending, agents, true)?;
            }
            let timestamp = time(raw)?;
            if timestamp <= pending.unwrap_or(start) || timestamp >= end {
                return Err(invalid(
                    "Inline timestamps must increase strictly inside the cue interval.",
                ));
            }
            pending = Some(timestamp);
        } else if let Some(name) = raw.strip_prefix('/') {
            if name == "ruby" && stack.last().is_some_and(|tag| tag_name(&tag.tag) == "rt") {
                stack.pop();
            }
            if stack.last().is_none_or(|tag| tag_name(&tag.tag) != name) {
                return Err(invalid("Mismatched span end tag."));
            }
            // Empty spans still carry source grouping and annotations.
            let path = serde_json::to_value(&stack).unwrap();
            if segments.last().is_none_or(|segment| {
                segment
                    .attributes
                    .get("webvtt:tags")
                    .and_then(serde_json::Value::as_array)
                    .is_none_or(|tags| !tags.starts_with(path.as_array().unwrap()))
            }) {
                emit(&mut buffer, &mut segments, &stack, pending, agents, true)?;
            }
            stack.pop();
        } else {
            check_tag(raw)?;
            if tag_name(raw) == "rt" && stack.last().is_none_or(|tag| tag_name(&tag.tag) != "ruby")
            {
                return Err(invalid("Ruby text must be inside a ruby span."));
            }
            if stack.len() >= 64 {
                return Err(Error::new("WEBVTT_TOO_DEEP", "Span nesting exceeds 64."));
            }
            group += 1;
            stack.push(Tag {
                group,
                tag: raw.into(),
            });
        }
        rest = tail;
    }
    buffer.push_str(rest);
    let force = segments.is_empty()
        || segments
            .last()
            .is_some_and(|segment| segment.start_us != pending);
    emit(&mut buffer, &mut segments, &stack, pending, agents, force)?;
    if !(stack.is_empty() || stack.len() == 1 && tag_name(&stack[0].tag) == "v") {
        return Err(invalid(
            "Unclosed span; only a sole voice span may omit its end tag.",
        ));
    }
    Ok(segments)
}

pub fn parse(input: &str) -> Result<ParseResult, Error> {
    if input.contains('\0') {
        return Err(invalid("NUL characters are not accepted by this profile."));
    }
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = normalized.split('\n').peekable();
    let header = lines
        .next()
        .ok_or_else(|| invalid("Missing WEBVTT header."))?;
    let suffix = header
        .strip_prefix("WEBVTT")
        .filter(|suffix| suffix.is_empty() || suffix.starts_with([' ', '\t']))
        .ok_or_else(|| invalid("Missing WEBVTT header."))?;
    if suffix.contains("-->") {
        return Err(invalid("The header must not contain an arrow."));
    }
    let mut document = Document::default();
    let mut diagnostics = vec![];
    if input.contains('\r') {
        diagnostics.push(Diagnostic::new(
            "NEWLINES_NORMALIZED",
            "WebVTT line separators were normalized to LF.",
            None,
        ));
    }
    if !suffix.is_empty() {
        document
            .attributes
            .insert("webvtt:header".into(), suffix.into());
    }
    let mut metadata = vec![];
    while let Some(line) = lines.peek() {
        if line.is_empty() {
            lines.next();
            break;
        }
        if line.contains("-->") {
            return Err(invalid("A blank line must separate the header and cues."));
        }
        metadata.push(lines.next().unwrap());
    }
    if !metadata.is_empty() {
        document
            .attributes
            .insert("webvtt:header_metadata".into(), serde_json::json!(metadata));
        diagnostics.push(Diagnostic::new(
            "WEBVTT_HEADER_UNINTERPRETED",
            "Header metadata is retained; timestamp mappings are not applied.",
            None,
        ));
    }
    let mut blocks = vec![];
    while let Some(first) = lines.next() {
        if first.is_empty() {
            continue;
        }
        let mut block = vec![first];
        while lines.peek().is_some_and(|line| !line.is_empty()) {
            block.push(lines.next().unwrap());
        }
        if first == "NOTE"
            || (first.starts_with("NOTE ") || first.starts_with("NOTE\t"))
            || matches!(first, "STYLE" | "REGION")
        {
            if matches!(first, "STYLE" | "REGION") && !document.cues.is_empty() {
                return Err(invalid("STYLE and REGION blocks must precede cues."));
            }
            blocks.push(serde_json::json!({"before":document.cues.len(),"text":block.join("\n")}));
            diagnostics.push(Diagnostic::new(
                "WEBVTT_BLOCK_RETAINED",
                "Non-cue blocks are retained without interpreting CSS or region layout.",
                None,
            ));
            continue;
        }
        let (identifier, timing, body) = if first.contains("-->") {
            (None, first, &block[1..])
        } else {
            (
                Some(first),
                *block.get(1).ok_or_else(|| invalid("Missing cue timing."))?,
                &block[2..],
            )
        };
        let (start, right) = timing
            .split_once("-->")
            .ok_or_else(|| invalid("Missing cue arrow."))?;
        let mut right = right.trim_start().splitn(2, [' ', '\t']);
        let start = time(start.trim())?;
        let end = time(right.next().unwrap_or(""))?;
        if end <= start {
            return Err(invalid("A cue must end after its start."));
        }
        if document
            .cues
            .last()
            .is_some_and(|cue: &Cue| cue.start_us.is_some_and(|previous| previous > start))
        {
            return Err(invalid("Cue start times must not decrease."));
        }
        let mut attributes = Attributes::new();
        if let Some(identifier) = identifier {
            attributes.insert("webvtt:identifier".into(), identifier.into());
        }
        if let Some(settings) = right
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if settings.contains("-->") {
                return Err(invalid("Invalid cue settings."));
            }
            attributes.insert("webvtt:settings".into(), settings.into());
            diagnostics.push(Diagnostic::new(
                "WEBVTT_SETTINGS_RETAINED",
                "Cue settings are retained without interpreting layout.",
                None,
            ));
        }
        let segments = payload(&body.join("\n"), start, end, &mut document.agents)?;
        document.cues.push(Cue {
            id: format!("L{}", document.cues.len() + 1),
            start_us: Some(start),
            end_us: Some(end),
            segments,
            attributes,
            ..Cue::default()
        });
        if document.cues.len() > 10_000 {
            return Err(Error::new("TOO_MANY_CUES", "Document exceeds 10000 cues."));
        }
    }
    if !blocks.is_empty() {
        document
            .attributes
            .insert("webvtt:blocks".into(), blocks.into());
    }
    Ok(ParseResult {
        document,
        diagnostics,
    })
}

fn stamp(time: u64) -> String {
    let seconds = time / 1_000_000;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60,
        time / 1000 % 1000
    )
}
fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn string_attribute<'a>(attributes: &'a Attributes, key: &str) -> Result<Option<&'a str>, Error> {
    attributes
        .get(key)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| invalid("WebVTT string attributes must contain strings."))
        })
        .transpose()
}

pub fn export(document: &Document, policy: Policy) -> Result<ExportResult, Error> {
    let mut output = "WEBVTT".to_owned();
    let mut losses = vec![];
    if let Some(header) = string_attribute(&document.attributes, "webvtt:header")? {
        if !header.starts_with([' ', '\t'])
            || header.contains(['\n', '\r'])
            || header.contains("-->")
        {
            return Err(invalid("Invalid header suffix."));
        }
        output.push_str(header);
    }
    output.push('\n');
    if let Some(metadata) = document.attributes.get("webvtt:header_metadata") {
        let values: Vec<String> = serde_json::from_value(metadata.clone())
            .map_err(|_| invalid("Invalid header metadata."))?;
        for value in values {
            if value.is_empty() || value.contains(['\n', '\r']) || value.contains("-->") {
                return Err(invalid("Invalid header metadata line."));
            }
            output.push_str(&value);
            output.push('\n');
        }
    }
    output.push('\n');
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Block {
        before: usize,
        text: String,
    }
    let blocks: Vec<Block> = document
        .attributes
        .get("webvtt:blocks")
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()
        .map_err(|_| invalid("Invalid non-cue blocks."))?
        .unwrap_or_default();
    if blocks
        .iter()
        .any(|block| block.before > document.cues.len())
    {
        return Err(invalid("A block position exceeds the cue count."));
    }
    for position in 0..=document.cues.len() {
        for block in blocks.iter().filter(|block| block.before == position) {
            if block.text.contains(['\r', '\0']) || block.text.contains("\n\n") {
                return Err(invalid("Invalid retained block."));
            }
            let first = block.text.lines().next().unwrap_or("");
            if !(first == "NOTE"
                || (first.starts_with("NOTE ") || first.starts_with("NOTE\t"))
                || position == 0 && matches!(first, "STYLE" | "REGION"))
            {
                return Err(invalid("Invalid retained block kind or position."));
            }
            output.push_str(&block.text);
            output.push_str("\n\n");
        }
        let Some(cue) = document.cues.get(position) else {
            break;
        };
        let (Some(start), Some(end)) = (cue.start_us, cue.end_us) else {
            return Err(Error::new(
                "MISSING_TIMING",
                "WebVTT requires explicit cue starts and ends.",
            ));
        };
        if let Some(identifier) = string_attribute(&cue.attributes, "webvtt:identifier")? {
            if identifier.is_empty()
                || identifier.contains(['\r', '\n'])
                || identifier.contains("-->")
                || identifier == "NOTE"
                || (identifier.starts_with("NOTE ") || identifier.starts_with("NOTE\t"))
                || matches!(identifier, "STYLE" | "REGION")
            {
                return Err(invalid("Invalid cue identifier."));
            }
            output.push_str(identifier);
            output.push('\n');
        }
        output.push_str(&format!("{} --> {}", stamp(start), stamp(end)));
        if let Some(settings) = string_attribute(&cue.attributes, "webvtt:settings")? {
            if settings.contains(['\r', '\n'])
                || settings.contains("-->")
                || settings.starts_with([' ', '\t'])
            {
                return Err(invalid("Invalid retained settings."));
            }
            output.push(' ');
            output.push_str(settings);
        }
        output.push('\n');
        let mut stack: Vec<Tag> = vec![];
        let mut previous = None;
        let mut next_group = 0;
        let text = cue.text();
        if text.contains(['\r', '\0'])
            || text.contains("\n\n")
            || text.starts_with('\n')
            || text.ends_with('\n')
        {
            return Err(Error::new("UNREPRESENTABLE_TEXT", "WebVTT cue payloads cannot preserve embedded blank lines, boundary newlines, carriage returns, or NUL."));
        }
        for segment in &cue.segments {
            let tags: Vec<Tag> = if let Some(value) = segment.attributes.get("webvtt:tags") {
                serde_json::from_value(value.clone()).map_err(|_| invalid("Invalid span paths."))?
            } else {
                let references = if segment.agent_ids.is_empty() {
                    &cue.agent_ids
                } else {
                    &segment.agent_ids
                };
                let mut names = vec![];
                if references.len() == 1 {
                    let agent = document
                        .agents
                        .iter()
                        .find(|agent| agent.id == references[0])
                        .ok_or_else(|| invalid("Unknown vocal agent."))?;
                    names.push(format!("v {}", agent.name.as_deref().unwrap_or(&agent.id)));
                } else if references.len() > 1 {
                    losses.push(Diagnostic::new(
                        "VOCAL_AGENTS_DROPPED",
                        "A WebVTT voice span cannot express simultaneous vocal-agent references.",
                        Some(&cue.id),
                    ));
                }
                if let Some(language) = segment
                    .language
                    .as_ref()
                    .or(cue.language.as_ref())
                    .or(document.language.as_ref())
                {
                    names.push(format!("lang {language}"));
                }
                let mut tags = vec![];
                for (index, name) in names.into_iter().enumerate() {
                    if let Some(tag) = stack.get(index).filter(|tag| tag.tag == name) {
                        tags.push(tag.clone());
                    } else {
                        next_group += 1;
                        tags.push(Tag {
                            group: next_group,
                            tag: name,
                        });
                    }
                }
                tags
            };
            if tags.len() > 64 {
                return Err(Error::new("WEBVTT_TOO_DEEP", "Span nesting exceeds 64."));
            }
            for tag in &tags {
                check_tag(&tag.tag)?;
                next_group = next_group.max(tag.group);
            }
            let shared = stack.iter().zip(&tags).take_while(|(a, b)| a == b).count();
            for tag in stack[shared..].iter().rev() {
                output.push_str(&format!("</{}>", tag_name(&tag.tag)));
            }
            for tag in &tags[shared..] {
                output.push_str(&format!("<{}>", tag.tag));
            }
            stack = tags;
            if let Some(timestamp) = segment.start_us {
                let encoded = timestamp / 1000 * 1000;
                if previous.is_none() && encoded == start / 1000 * 1000 {
                    losses.push(Diagnostic::new("SEGMENT_START_IMPLICIT", "WebVTT forbids an inline timestamp equal to the cue start; this explicit segment anchor is omitted.", Some(&cue.id)));
                } else if previous != Some(encoded) {
                    output.push_str(&format!("<{}>", stamp(timestamp)));
                    previous = Some(encoded);
                }
            }
            output.push_str(&escaped(&segment.text));
        }
        for tag in stack.iter().rev() {
            output.push_str(&format!("</{}>", tag_name(&tag.tag)));
        }
        output.push_str("\n\n");
    }
    let mut projected = parse(&output)?.document;
    // New source paths are serialization provenance, not lost canonical information.
    for (projected_cue, cue) in projected.cues.iter_mut().zip(&document.cues) {
        for (projected_segment, segment) in projected_cue.segments.iter_mut().zip(&cue.segments) {
            if !segment.attributes.contains_key("webvtt:tags") {
                projected_segment.attributes.remove("webvtt:tags");
            }
        }
    }
    if projected != *document {
        losses.push(Diagnostic::new("WEBVTT_PROJECTION_CHANGED","WebVTT cannot preserve all canonical metadata, auxiliary relationships, annotation identities, segment ends, or this segmentation; the reparsed projection differs.",None));
    }
    if document.cues.iter().any(|cue| {
        [cue.start_us, cue.end_us]
            .into_iter()
            .flatten()
            .chain(
                cue.segments
                    .iter()
                    .flat_map(|segment| [segment.start_us, segment.end_us].into_iter().flatten()),
            )
            .any(|time| time % 1000 != 0)
    }) {
        losses.push(Diagnostic::new(
            "TIME_PRECISION_REDUCED",
            "WebVTT floors timestamps to milliseconds.",
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
