use crate::{
    Agent, Attributes, Auxiliary, Cue, Diagnostic, Document, Error, ExportResult, ParseResult,
    Policy, Segment, MAX_SAFE_TIME_US,
};
use roxmltree::{Node, NodeId};
use std::collections::{HashMap, HashSet};

const TT: &str = "http://www.w3.org/ns/ttml";
const TTM: &str = "http://www.w3.org/ns/ttml#metadata";
const TTP: &str = "http://www.w3.org/ns/ttml#parameter";
const XML: &str = "http://www.w3.org/XML/1998/namespace";
const ITUNES: &str = "http://music.apple.com/lyric-ttml-internal";
const AMLL: &str = "http://www.example.com/ns/amll";

fn unsupported(message: impl Into<String>) -> Error {
    Error::new("UNSUPPORTED_TTML", message)
}
fn time_error() -> Error {
    Error::new(
        "INVALID_TTML_TIME",
        "Invalid or out-of-range nonnegative TTML time.",
    )
}

fn decimal(value: &str, factor: u128) -> Result<u64, Error> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty()
        || !whole.bytes().all(|c| c.is_ascii_digit())
        || !fraction.bytes().all(|c| c.is_ascii_digit())
        || value.ends_with('.')
        || fraction.len() > 18
    {
        return Err(time_error());
    }
    let denominator = 10u128.pow(fraction.len() as u32);
    let numerator = whole
        .parse::<u128>()
        .ok()
        .and_then(|n| n.checked_mul(denominator))
        .and_then(|n| {
            n.checked_add(if fraction.is_empty() {
                0
            } else {
                fraction.parse().ok()?
            })
        })
        .and_then(|n| n.checked_mul(factor))
        .ok_or_else(time_error)?;
    if numerator % denominator != 0 {
        return Err(Error::new(
            "UNSUPPORTED_PRECISION",
            "Time is not exactly representable in integer microseconds.",
        ));
    }
    u64::try_from(numerator / denominator)
        .ok()
        .filter(|n| *n <= MAX_SAFE_TIME_US)
        .ok_or_else(time_error)
}

fn time(value: &str, music: bool) -> Result<u64, Error> {
    if value.contains(':') {
        let parts: Vec<_> = value.split(':').collect();
        if parts.len() == 4 {
            return Err(unsupported("Frame-clock timing is not implemented."));
        }
        let (hours, minutes, seconds) = match parts.as_slice() {
            [hours, minutes, seconds] => (*hours, *minutes, *seconds),
            [minutes, seconds] if music => ("0", *minutes, *seconds),
            _ => return Err(time_error()),
        };
        if !hours.bytes().all(|c| c.is_ascii_digit())
            || hours.len() < if music { 1 } else { 2 }
            || minutes.len() != 2
            || seconds.split('.').next().unwrap_or("").len() != 2
        {
            return Err(time_error());
        }
        let h = decimal(hours, 3_600_000_000)?;
        let m = decimal(minutes, 60_000_000)?;
        let s = decimal(seconds, 1_000_000)?;
        if m >= 3_600_000_000 || s >= 60_000_000 {
            return Err(time_error());
        }
        return h
            .checked_add(m)
            .and_then(|n| n.checked_add(s))
            .filter(|n| *n <= MAX_SAFE_TIME_US)
            .ok_or_else(time_error);
    }
    for (suffix, factor) in [
        ("ms", 1_000),
        ("h", 3_600_000_000),
        ("m", 60_000_000),
        ("s", 1_000_000),
    ] {
        if let Some(value) = value.strip_suffix(suffix) {
            return decimal(value, factor);
        }
    }
    if value.ends_with(['f', 't']) {
        return Err(unsupported("Frame/tick timing is not implemented."));
    }
    if music {
        decimal(value, 1_000_000)
    } else {
        Err(time_error())
    }
}

#[derive(Clone, Copy, Default)]
struct Timing {
    start: u64,
    end: Option<u64>,
    inline_timed: bool,
}

struct Reader {
    music: bool,
    document: Document,
    diagnostics: Vec<Diagnostic>,
    timings: HashMap<NodeId, Timing>,
    identities: HashMap<NodeId, String>,
    reserved: HashSet<String>,
    counter: usize,
    keys: HashMap<String, String>,
}

impl Reader {
    fn generated(&mut self, prefix: &str) -> String {
        loop {
            self.counter += 1;
            let id = format!("{prefix}{}", self.counter);
            if self.reserved.insert(id.clone()) {
                return id;
            }
        }
    }

    fn identity(&mut self, node: Node<'_, '_>, prefix: &str) -> String {
        if let Some(id) = self.identities.get(&node.id()) {
            return id.clone();
        }
        let id = node
            .attribute((XML, "id"))
            .or_else(|| node.attribute((ITUNES, "key")))
            .map(str::to_owned)
            .unwrap_or_else(|| self.generated(prefix));
        self.identities.insert(node.id(), id.clone());
        id
    }

    fn resolve(
        &mut self,
        node: Node<'_, '_>,
        parent: Timing,
        base: u64,
        depth: usize,
    ) -> Result<Timing, Error> {
        if depth > 64 {
            return Err(Error::new(
                "XML_TOO_DEEP",
                "XML nesting exceeds 64 elements.",
            ));
        }
        let offset = |value| time(value, self.music);
        let own = node.attribute("begin").is_some()
            || node.attribute("end").is_some()
            || node.attribute("dur").is_some();
        let origin = if self.music { 0 } else { base };
        let add = |a: u64, b: u64| {
            a.checked_add(b)
                .filter(|n| *n <= MAX_SAFE_TIME_US)
                .ok_or_else(time_error)
        };
        let start = if let Some(value) = node.attribute("begin") {
            add(origin, offset(value)?)?
        } else {
            base
        };
        let mut end = node
            .attribute("end")
            .map(|v| offset(v).and_then(|n| add(origin, n)))
            .transpose()?;
        if let Some(duration) = node.attribute("dur") {
            let duration_end = add(start, offset(duration)?)?;
            end = Some(end.map_or(duration_end, |end| end.min(duration_end)));
        }
        if let Some(limit) = parent.end {
            end = Some(end.map_or(limit, |end| end.min(limit)));
        }
        if start < parent.start || end.is_some_and(|end| end < start) {
            return Err(Error::new(
                "INVALID_INTERVAL",
                "TTML interval lies outside its parent or ends before it starts.",
            ));
        }
        let timing = Timing {
            start,
            end,
            inline_timed: own || parent.inline_timed,
        };
        let sequential = match node.attribute("timeContainer") {
            None | Some("par") => false,
            Some("seq") if !self.music => true,
            _ => {
                return Err(unsupported(
                    "Unsupported timeContainer for the selected TTML dialect.",
                ))
            }
        };
        let mut cursor = start;
        let mut max_end = Some(start);
        let mut child_count = 0;
        let children: Vec<_> = node
            .children()
            .filter(|n| {
                n.is_element()
                    && (n.tag_name().namespace() == Some(TT)
                        || self.music && n.tag_name().namespace() == Some(ITUNES))
                    && matches!(n.tag_name().name(), "body" | "div" | "p" | "span" | "br")
            })
            .collect();
        for (index, child) in children.iter().enumerate() {
            let cue_like = node.tag_name().name() == "p"
                || self.music
                    && Self::roles(node)
                        .iter()
                        .any(|r| matches!(r.as_str(), "background" | "x-translation" | "x-roman"));
            let child_parent = Timing {
                inline_timed: !cue_like && node.tag_name().name() == "span" && timing.inline_timed
                    || sequential,
                ..timing
            };
            let child_timing = self.resolve(
                *child,
                child_parent,
                if sequential { cursor } else { start },
                depth + 1,
            )?;
            child_count += 1;
            max_end = match (max_end, child_timing.end) {
                (Some(a), Some(b)) => Some(a.max(b)),
                _ => None,
            };
            if sequential && index + 1 < children.len() {
                cursor = child_timing
                    .end
                    .ok_or_else(|| unsupported("A sequential child has no finite resolved end."))?;
            }
        }
        let has_text = node.children().any(|child| {
            child.is_text()
                && child
                    .text()
                    .is_some_and(|text| !text.chars().all(xml_space))
        });
        let result = Timing {
            end: if end.is_none() && child_count > 0 && !has_text {
                max_end
            } else {
                end
            },
            ..timing
        };
        self.timings.insert(node.id(), result);
        Ok(result)
    }

    fn attributes(&mut self, node: Node<'_, '_>) -> Attributes {
        let mut unknown = serde_json::Map::new();
        for attribute in node.attributes() {
            let known = match attribute.namespace() {
                None => {
                    matches!(attribute.name(), "begin" | "end" | "dur" | "timeContainer")
                        || node.has_tag_name((TTM, "agent")) && attribute.name() == "type"
                        || node.has_tag_name((ITUNES, "text")) && attribute.name() == "for"
                }
                Some(XML) => matches!(attribute.name(), "id" | "lang" | "space"),
                Some(TTM) => matches!(attribute.name(), "agent" | "role"),
                Some(ITUNES) => attribute.name() == "key",
                _ => false,
            };
            if !known {
                let key = format!(
                    "{{{}}}{}",
                    attribute.namespace().unwrap_or(""),
                    attribute.name()
                );
                unknown.insert(key, attribute.value().into());
            }
        }
        let mut attributes = Attributes::new();
        if !unknown.is_empty() {
            self.diagnostics.push(Diagnostic::new("TTML_ATTRIBUTES_RETAINED", "Uninterpreted XML attributes retained as expanded namespace names; their rendering semantics are not implemented.",None));
            attributes.insert("ttml:attributes".into(), unknown.into());
        }
        attributes
    }

    fn roles(node: Node<'_, '_>) -> Vec<String> {
        node.attribute((TTM, "role"))
            .unwrap_or("")
            .split_whitespace()
            .map(|role| {
                match role {
                    "x-bg" => "background",
                    "x-lead" => "lead",
                    "x-instrumental" => "instrumental",
                    "x-syllable" => "syllable",
                    other => other,
                }
                .to_owned()
            })
            .collect()
    }

    fn inline(
        &mut self,
        node: Node<'_, '_>,
        cue: &mut Cue,
        inherited: &Segment,
        defaults: &mut Vec<bool>,
        outer: bool,
        auxiliary: bool,
    ) -> Result<(), Error> {
        let preserve = match node.ancestors().find_map(|n| n.attribute((XML, "space"))) {
            Some("preserve") => true,
            Some("default") => false,
            None => false,
            _ => return Err(unsupported("Unknown xml:space value.")),
        };
        let mut segment = inherited.clone();
        if !outer {
            if let Some(language) = node.attribute((XML, "lang")) {
                segment.language = Some(language.into());
            }
            if let Some(agents) = node.attribute((TTM, "agent")) {
                segment.agent_ids = agents.split_whitespace().map(str::to_owned).collect();
            }
            segment.roles.extend(Self::roles(node));
            let timing = self.timings.get(&node.id()).copied().unwrap_or_default();
            if timing.inline_timed {
                segment.start_us = Some(timing.start);
                segment.end_us = timing.end;
            }
            let attributes = self.attributes(node);
            if !attributes.is_empty() && !segment.attributes.is_empty() {
                let inherited = std::mem::take(&mut segment.attributes);
                segment.attributes.insert(
                    "ttml:ancestor_attributes".into(),
                    serde_json::to_value(inherited).unwrap(),
                );
            }
            segment.attributes.extend(attributes);
        }
        let children: Vec<_> = node.children().collect();
        let has_elements = children.iter().any(Node::is_element);
        if has_elements && node.attribute((XML, "id")).is_some() && !outer {
            return Err(unsupported("An identified inline container with child elements cannot yet be represented without losing its grouping identity."));
        }
        if !has_elements && !outer {
            segment.id = node.attribute((XML, "id")).map(str::to_owned);
        }
        let mut emitted = false;
        for child in children {
            if child.is_text() {
                let raw = child.text().unwrap_or("");
                if self.music && has_elements && raw.chars().all(xml_space) {
                    continue;
                }
                let mut text = String::new();
                let mut previous_space = cue
                    .segments
                    .iter()
                    .rev()
                    .find_map(|s| s.text.chars().next_back())
                    .is_none_or(xml_space);
                for character in raw.chars() {
                    if !preserve && xml_space(character) {
                        if !previous_space {
                            text.push(' ');
                        }
                        previous_space = true;
                    } else {
                        text.push(character);
                        previous_space = xml_space(character);
                    }
                }
                if emitted && !has_elements {
                    cue.segments.last_mut().unwrap().text.push_str(&text);
                } else {
                    cue.segments.push(Segment {
                        text,
                        ..segment.clone()
                    });
                    defaults.push(!preserve);
                }
                emitted = true;
            } else if child.is_element() {
                if child.tag_name().namespace() != Some(TT)
                    && !(self.music && child.tag_name().namespace() == Some(ITUNES))
                {
                    return Err(unsupported("Foreign inline elements are not implemented."));
                }
                match child.tag_name().name() {
                    "span" => {
                        let roles = Self::roles(child);
                        if self.music && !auxiliary && roles.iter().any(|role| role == "background")
                        {
                            let id = self.identity(child, "BG");
                            let position = self.document.cues.len();
                            let mut background = self.cue(child, id, false)?;
                            background.parent_id = Some(cue.id.clone());
                            self.document.cues.insert(position, background);
                        } else if self.music
                            && roles
                                .iter()
                                .any(|role| role == "x-translation" || role == "x-roman")
                        {
                            let kind = if roles.iter().any(|r| r == "x-translation") {
                                "translation"
                            } else {
                                "romanization"
                            };
                            let id = self.identity(child, "A");
                            if auxiliary {
                                return Err(unsupported(
                                    "Nested auxiliary tracks are not implemented.",
                                ));
                            }
                            let mut content = self.cue(child, id.clone(), true)?;
                            apply_auxiliary_annotations(child, &mut content);
                            self.document.auxiliary.push(Auxiliary {
                                id,
                                kind: kind.into(),
                                language: content.language,
                                target_ids: vec![cue.id.clone()],
                                segments: content.segments,
                                attributes: content.attributes,
                            });
                        } else {
                            self.inline(child, cue, &segment, defaults, false, auxiliary)?;
                        }
                    }
                    "br" => {
                        cue.segments.push(Segment {
                            text: "\n".into(),
                            ..segment.clone()
                        });
                        defaults.push(false);
                    }
                    _ => {
                        return Err(unsupported(
                            "Only span and br are accepted inside lyric text.",
                        ))
                    }
                }
            } else if child.is_pi() {
                return Err(unsupported(
                    "XML processing instructions are not supported.",
                ));
            }
        }
        if !emitted && !has_elements {
            cue.segments.push(segment);
            defaults.push(!preserve);
        }
        Ok(())
    }

    fn cue(&mut self, node: Node<'_, '_>, id: String, auxiliary: bool) -> Result<Cue, Error> {
        if let Some(key) = node.attribute((ITUNES, "key")) {
            if self.keys.insert(key.into(), id.clone()).is_some() {
                return Err(Error::new("INVALID_ID", "Duplicate iTunes lyric key."));
            }
        }
        let timing = self.timings.get(&node.id()).copied().unwrap_or_default();
        let mut cue = Cue {
            id,
            start_us: Some(timing.start),
            end_us: timing.end,
            language: node
                .ancestors()
                .take_while(|n| !n.has_tag_name((TT, "tt")))
                .find_map(|n| n.attribute((XML, "lang")))
                .map(str::to_owned),
            agent_ids: node
                .attribute((TTM, "agent"))
                .unwrap_or("")
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
            roles: Self::roles(node),
            attributes: self.attributes(node),
            ..Cue::default()
        };
        let mut defaults = vec![];
        // Cue-level annotations stay on the cue; only nested span annotations reach runs.
        let seed = Segment::default();
        let mut temporary = cue.clone();
        self.inline(node, &mut temporary, &seed, &mut defaults, true, auxiliary)?;
        cue.segments = temporary.segments;
        for (segment, default) in cue.segments.iter_mut().zip(defaults).rev() {
            if !default {
                break;
            }
            segment
                .text
                .truncate(segment.text.trim_end_matches(' ').len());
            if !segment.text.is_empty() {
                break;
            }
        }
        Ok(cue)
    }
}

fn xml_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

fn apply_auxiliary_annotations(node: Node<'_, '_>, content: &mut Cue) {
    let timed = ["begin", "end", "dur"]
        .iter()
        .any(|name| node.attribute(*name).is_some());
    for segment in &mut content.segments {
        if timed && segment.start_us.is_none() {
            segment.start_us = content.start_us;
            segment.end_us = content.end_us;
        }
        if segment.agent_ids.is_empty() {
            segment.agent_ids.clone_from(&content.agent_ids);
        }
        for role in &content.roles {
            if role != "x-translation" && role != "x-roman" && !segment.roles.contains(role) {
                segment.roles.push(role.clone());
            }
        }
    }
}

fn structure(root: Node<'_, '_>, music: bool) -> Result<(), Error> {
    let inline_parent = |parent: Node<'_, '_>| {
        parent.has_tag_name((TT, "p"))
            || parent.has_tag_name((TT, "span"))
            || music
                && (parent.has_tag_name((ITUNES, "text")) || parent.has_tag_name((ITUNES, "span")))
    };
    for node in root.descendants().filter(Node::is_element) {
        let parent = node.parent_element();
        let valid = match (node.tag_name().namespace(), node.tag_name().name()) {
            (Some(TT), "tt") => node == root,
            (Some(TT), "head" | "body") => parent == Some(root),
            (Some(TT), "metadata") => parent.is_some_and(|p| p.has_tag_name((TT, "head"))),
            (Some(TT), "div" | "p") => {
                parent.is_some_and(|p| p.has_tag_name((TT, "body")) || p.has_tag_name((TT, "div")))
            }
            (Some(TT), "span" | "br") => parent.is_some_and(inline_parent),
            (Some(TTM), "agent") => parent.is_some_and(|p| p.has_tag_name((TT, "metadata"))),
            (Some(TTM), "name") => parent.is_some_and(|p| p.has_tag_name((TTM, "agent"))),
            (Some(AMLL), "meta") if music => {
                parent.is_some_and(|p| p.has_tag_name((TT, "metadata")))
            }
            (Some(ITUNES), "iTunesMetadata") if music => {
                parent.is_some_and(|p| p.has_tag_name((TT, "metadata")))
            }
            (Some(ITUNES), "translations" | "transliterations") if music => {
                parent.is_some_and(|p| p.has_tag_name((ITUNES, "iTunesMetadata")))
            }
            (Some(ITUNES), "translation") if music => {
                parent.is_some_and(|p| p.has_tag_name((ITUNES, "translations")))
            }
            (Some(ITUNES), "transliteration") if music => {
                parent.is_some_and(|p| p.has_tag_name((ITUNES, "transliterations")))
            }
            (Some(ITUNES), "text") if music => parent.is_some_and(|p| {
                p.has_tag_name((ITUNES, "translation"))
                    || p.has_tag_name((ITUNES, "transliteration"))
            }),
            (Some(ITUNES), "span" | "br") if music => parent.is_some_and(inline_parent),
            _ => false,
        };
        if !valid {
            return Err(unsupported(format!(
                "Unsupported element or placement {{{}}}{}.",
                node.tag_name().namespace().unwrap_or(""),
                node.tag_name().name()
            )));
        }
        let is_text = matches!(node.tag_name().name(), "p" | "span" | "text" | "name");
        if !is_text
            && node.children().any(|child| {
                child.is_text()
                    && child
                        .text()
                        .is_some_and(|text| !text.chars().all(xml_space))
            })
        {
            return Err(unsupported(
                "Non-whitespace text outside a supported lyric/name element.",
            ));
        }
        if node.tag_name().name() == "br"
            && (node.attributes().len() > 0 || node.children().next().is_some())
        {
            return Err(unsupported(
                "Attributed or nonempty line-break elements are not implemented.",
            ));
        }
        if node.has_tag_name((TTM, "name")) && node.attributes().len() > 0 {
            return Err(unsupported(
                "Agent-name annotations are not yet represented.",
            ));
        }
        if (node.has_tag_name((TT, "body")) || node.has_tag_name((TT, "div")))
            && (node.attribute((TTM, "agent")).is_some() || node.attribute((TTM, "role")).is_some())
        {
            return Err(unsupported(
                "Container vocal/role annotations are not yet resolved.",
            ));
        }
    }
    if root
        .children()
        .filter(|node| node.has_tag_name((TT, "head")))
        .count()
        > 1
    {
        return Err(unsupported("Multiple TTML heads are not accepted."));
    }
    Ok(())
}

fn xml_document(input: &str) -> Result<roxmltree::Document<'_>, Error> {
    let mut rest = input;
    while let Some(index) = rest.find("<!") {
        rest = &rest[index..];
        let (start, ending) = if rest.starts_with("<!--") {
            (4, "-->")
        } else if rest.starts_with("<![CDATA[") {
            (9, "]]>")
        } else {
            if rest.starts_with("<!DOCTYPE") {
                return Err(Error::new(
                    "UNSAFE_XML",
                    "DTD declarations are not accepted.",
                ));
            }
            rest = &rest[2..];
            continue;
        };
        rest = match rest[start..].find(ending) {
            Some(end) => &rest[start + end + ending.len()..],
            None => break,
        };
    }
    let xml = roxmltree::Document::parse_with_options(
        input,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 100_000,
        },
    )
    .map_err(|error| {
        Error::new(
            if matches!(error, roxmltree::Error::NodesLimitReached) {
                "XML_TOO_LARGE"
            } else {
                "INVALID_XML"
            },
            error.to_string(),
        )
    })?;
    for node in xml.descendants() {
        if node.ancestors().take(67).count() > 65 {
            return Err(Error::new(
                "XML_TOO_DEEP",
                "XML nesting exceeds 64 elements.",
            ));
        }
        if node.is_pi() {
            return Err(unsupported("Processing instructions are not supported."));
        }
    }
    Ok(xml)
}

pub fn detect(input: &str) -> Result<(), Error> {
    let xml = xml_document(input)?;
    if !xml.root_element().has_tag_name((TT, "tt")) {
        return Err(Error::new(
            "UNSUPPORTED_FORMAT",
            "Expected a namespaced TTML root.",
        ));
    }
    if xml.descendants().any(|node| {
        node.is_element()
            && (node.tag_name().namespace() == Some(ITUNES)
                || node.tag_name().namespace() == Some(AMLL)
                || node
                    .attributes()
                    .any(|attribute| attribute.namespace() == Some(ITUNES))
                || node.attribute((TTM, "role")).is_some_and(|role| {
                    role.split_whitespace()
                        .any(|role| matches!(role, "x-bg" | "x-translation" | "x-roman"))
                }))
    }) {
        return Err(Error::new("AMBIGUOUS_TTML_DIALECT","Music TTML markers found: select ttml_lyrics for absolute music timing or ttml for standard relative timing."));
    }
    Ok(())
}

pub fn parse(input: &str, music: bool) -> Result<ParseResult, Error> {
    let xml = xml_document(input)?;
    let root = xml.root_element();
    if root.tag_name().namespace() != Some(TT) || root.tag_name().name() != "tt" {
        return Err(Error::new(
            "UNSUPPORTED_FORMAT",
            "Expected a namespaced TTML tt root.",
        ));
    }
    structure(root, music)?;
    let mut reader = Reader {
        music,
        document: Document::default(),
        diagnostics: vec![],
        timings: HashMap::new(),
        identities: HashMap::new(),
        reserved: HashSet::new(),
        counter: 0,
        keys: HashMap::new(),
    };
    let mut xml_ids = HashSet::new();
    for node in xml.descendants() {
        if node.ancestors().take(67).count() > 65 {
            return Err(Error::new(
                "XML_TOO_DEEP",
                "XML nesting exceeds 64 elements.",
            ));
        }
        if node.is_pi() {
            return Err(unsupported(
                "XML processing instructions are not supported.",
            ));
        }
        if !node.is_element() {
            continue;
        }
        if let Some(id) = node.attribute((XML, "id")) {
            if id.is_empty() || !xml_ids.insert(id) {
                return Err(Error::new(
                    "INVALID_ID",
                    "XML IDs must be nonempty and unique.",
                ));
            }
        }
        if let Some(id) = node.attribute((XML, "id")) {
            reader.reserved.insert(id.into());
        }
        if let Some(key) = node.attribute((ITUNES, "key")) {
            reader.reserved.insert(key.into());
        }
        if node.attribute("condition").is_some()
            || node.attribute("repeatCount").is_some()
            || node.attribute("repeatDur").is_some()
            || node.attribute("speed").is_some()
        {
            return Err(unsupported(
                "Conditional/repeating timing is not implemented.",
            ));
        }
        if node
            .attribute((TTP, "timeBase"))
            .is_some_and(|value| value != "media")
        {
            return Err(unsupported("Only the media time base is implemented."));
        }
        if node
            .attribute((XML, "space"))
            .is_some_and(|value| value != "default" && value != "preserve")
        {
            return Err(unsupported("Unknown xml:space value."));
        }
    }
    reader.document.language = root.attribute((XML, "lang")).map(str::to_owned);
    reader.document.attributes = reader.attributes(root);
    reader.resolve(root, Timing::default(), 0, 0)?;
    let bodies: Vec<_> = root
        .children()
        .filter(|n| n.has_tag_name((TT, "body")))
        .collect();
    if bodies.len() != 1 {
        return Err(unsupported("TTML must have exactly one body."));
    }
    for node in root.descendants().filter(Node::is_element) {
        match (node.tag_name().namespace(), node.tag_name().name()) {
            (Some(TT), "tt" | "head" | "metadata" | "body" | "div" | "p" | "span" | "br") => {}
            (Some(TTM), "agent" | "name") => {}
            (Some(AMLL), "meta") if music => {}
            (
                Some(ITUNES),
                "iTunesMetadata" | "translations" | "translation" | "transliterations"
                | "transliteration" | "text" | "span" | "br",
            ) if music => {}
            _ => {
                return Err(unsupported(format!(
                    "Unsupported TTML element {{{}}}{}.",
                    node.tag_name().namespace().unwrap_or(""),
                    node.tag_name().name()
                )))
            }
        }
    }
    for node in root
        .descendants()
        .filter(|n| n.has_tag_name((TTM, "agent")))
    {
        let id = node
            .attribute((XML, "id"))
            .ok_or_else(|| unsupported("A vocal agent needs xml:id."))?;
        let names: Vec<_> = node
            .children()
            .filter(|n| n.has_tag_name((TTM, "name")))
            .collect();
        if names.len() > 1 {
            return Err(unsupported("Multiple agent names are not yet represented."));
        }
        let attributes = reader.attributes(node);
        reader.document.agents.push(Agent {
            id: id.into(),
            name: names.first().map(|n| n.text().unwrap_or("").to_owned()),
            kind: node.attribute("type").map(str::to_owned),
            attributes,
        });
    }
    for node in bodies[0]
        .descendants()
        .filter(|n| n.has_tag_name((TT, "p")))
    {
        if node
            .ancestors()
            .skip(1)
            .any(|ancestor| ancestor.has_tag_name((TT, "p")))
        {
            return Err(unsupported("Nested paragraphs are not allowed."));
        }
        let id = reader.identity(node, "L");
        let position = reader.document.cues.len();
        let cue = reader.cue(node, id, false)?;
        reader.document.cues.insert(position, cue);
    }
    for node in root
        .descendants()
        .filter(|n| n.has_tag_name((AMLL, "meta")))
    {
        reader.document.metadata.push(crate::Metadata {
            key: node
                .attribute("key")
                .ok_or_else(|| unsupported("AMLL metadata needs key."))?
                .into(),
            value: node
                .attribute("value")
                .ok_or_else(|| unsupported("AMLL metadata needs value."))?
                .into(),
        });
    }
    for node in root
        .descendants()
        .filter(|n| n.has_tag_name((ITUNES, "text")))
    {
        let parent = node
            .parent_element()
            .ok_or_else(|| unsupported("Auxiliary text needs a track parent."))?;
        let kind = match parent.tag_name().name() {
            "translation" => "translation",
            "transliteration" => "romanization",
            _ => return Err(unsupported("Unknown auxiliary track.")),
        };
        let target = node
            .attribute("for")
            .and_then(|key| reader.keys.get(key))
            .cloned()
            .ok_or_else(|| Error::new("UNKNOWN_TARGET", "Auxiliary lyric key does not exist."))?;
        let id = reader.identity(node, "A");
        reader.resolve(node, Timing::default(), 0, 0)?;
        let mut content = reader.cue(node, id.clone(), true)?;
        apply_auxiliary_annotations(node, &mut content);
        let track_attributes = reader.attributes(parent);
        if !track_attributes.is_empty() {
            content.attributes.insert(
                "ttml:track_attributes".into(),
                serde_json::to_value(track_attributes).unwrap(),
            );
        }
        reader.document.auxiliary.push(Auxiliary {
            id,
            kind: kind.into(),
            language: node
                .attribute((XML, "lang"))
                .or_else(|| parent.attribute((XML, "lang")))
                .map(str::to_owned),
            target_ids: vec![target],
            segments: content.segments,
            attributes: content.attributes,
        });
    }
    // Container-only attributes cannot be projected onto an arbitrary descendant cue.
    for node in root
        .descendants()
        .filter(|n| n.has_tag_name((TT, "body")) || n.has_tag_name((TT, "div")))
    {
        let attrs = reader.attributes(node);
        if !attrs.is_empty() || node.attribute((XML, "id")).is_some() {
            let cue_ids: Vec<_> = node
                .descendants()
                .filter_map(|n| reader.identities.get(&n.id()))
                .filter(|id| reader.document.cues.iter().any(|cue| &cue.id == *id))
                .collect();
            reader
                .document
                .attributes
                .entry("ttml:containers".into())
                .or_insert_with(|| serde_json::json!([]))
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"element":node.tag_name().name(),"id":node.attribute((XML,"id")),"cue_ids":cue_ids,"attributes":attrs}));
        }
    }
    reader.diagnostics.push(Diagnostic::new("TTML_TIMING_RESOLVED","TTML intervals were resolved using the explicitly selected standard-relative or lyrics-absolute timing rules; no missing end was guessed from the next lyric.",None));
    Ok(ParseResult {
        document: reader.document,
        diagnostics: reader.diagnostics,
    })
}

fn escaped(value: &str) -> Result<String, Error> {
    let mut output = String::new();
    for c in value.chars() {
        match c {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\r' => output.push_str("&#13;"),
            '\n' => output.push_str("&#10;"),
            '\t' => output.push_str("&#9;"),
            c if c == '\u{fffe}' || c == '\u{ffff}' || (c as u32) < 32 => {
                return Err(Error::new(
                    "UNREPRESENTABLE_XML",
                    "Text contains a character forbidden by XML 1.0.",
                ))
            }
            c => output.push(c),
        }
    }
    Ok(output)
}

fn attribute(output: &mut String, name: &str, value: &str) -> Result<(), Error> {
    output.push_str(&format!(" {name}=\"{}\"", escaped(value)?));
    Ok(())
}
fn stamp(time: u64) -> String {
    format!("{}.{:06}s", time / 1_000_000, time % 1_000_000)
}
fn roles(values: &[String]) -> String {
    values
        .iter()
        .map(|r| match r.as_str() {
            "background" => "x-bg",
            "lead" => "x-lead",
            "instrumental" => "x-instrumental",
            "syllable" => "x-syllable",
            r => r,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn write_segments(
    output: &mut String,
    segments: &[Segment],
    origin: u64,
    music: bool,
) -> Result<(), Error> {
    for segment in segments {
        output.push_str("<span");
        if let Some(id) = &segment.id {
            attribute(output, "xml:id", id)?;
        }
        for (name, value) in [("begin", segment.start_us), ("end", segment.end_us)] {
            if let Some(value) = value {
                let relative = if music {
                    value
                } else {
                    value.checked_sub(origin).ok_or_else(|| {
                        unsupported("A segment begins before its containing paragraph.")
                    })?
                };
                attribute(output, name, &stamp(relative))?;
            }
        }
        if let Some(language) = &segment.language {
            attribute(output, "xml:lang", language)?;
        }
        if !segment.agent_ids.is_empty() {
            attribute(output, "ttm:agent", &segment.agent_ids.join(" "))?;
        }
        if !segment.roles.is_empty() {
            attribute(output, "ttm:role", &roles(&segment.roles))?;
        }
        output.push_str(&format!(">{}</span>", escaped(&segment.text)?));
    }
    Ok(())
}

fn auxiliary_supported(auxiliary: &Auxiliary, document: &Document, music: bool) -> bool {
    music
        && matches!(auxiliary.kind.as_str(), "translation" | "romanization")
        && auxiliary.target_ids.len() == 1
        && document
            .cues
            .iter()
            .any(|cue| cue.id == auxiliary.target_ids[0])
}

pub fn export(document: &Document, music: bool, policy: Policy) -> Result<ExportResult, Error> {
    let mut losses = vec![];
    let mut output = format!(
        "<tt xmlns=\"{TT}\" xmlns:ttm=\"{TTM}\" xmlns:itunes=\"{ITUNES}\" xmlns:amll=\"{AMLL}\" xml:space=\"preserve\""
    );
    if let Some(language) = &document.language {
        attribute(&mut output, "xml:lang", language)?;
    }
    output.push_str("><head><metadata>");
    for agent in &document.agents {
        output.push_str("<ttm:agent");
        attribute(&mut output, "xml:id", &agent.id)?;
        if let Some(kind) = &agent.kind {
            attribute(&mut output, "type", kind)?;
        }
        output.push('>');
        if let Some(name) = &agent.name {
            output.push_str(&format!("<ttm:name>{}</ttm:name>", escaped(name)?));
        }
        output.push_str("</ttm:agent>");
    }
    if music {
        for metadata in &document.metadata {
            output.push_str("<amll:meta");
            attribute(&mut output, "key", &metadata.key)?;
            attribute(&mut output, "value", &metadata.value)?;
            output.push_str("/>");
        }
        output.push_str("<itunes:iTunesMetadata>");
        for auxiliary in &document.auxiliary {
            if !auxiliary_supported(auxiliary, document, music) {
                continue;
            }
            let (plural, singular) = if auxiliary.kind == "translation" {
                ("translations", "translation")
            } else {
                ("transliterations", "transliteration")
            };
            output.push_str(&format!("<itunes:{plural}><itunes:{singular}><itunes:text"));
            attribute(&mut output, "xml:id", &auxiliary.id)?;
            attribute(&mut output, "for", &auxiliary.target_ids[0])?;
            if let Some(language) = &auxiliary.language {
                attribute(&mut output, "xml:lang", language)?;
            }
            output.push('>');
            write_segments(&mut output, &auxiliary.segments, 0, true)?;
            output.push_str(&format!(
                "</itunes:text></itunes:{singular}></itunes:{plural}>"
            ));
        }
        output.push_str("</itunes:iTunesMetadata>");
    }
    output.push_str("</metadata></head><body><div>");
    let indices: HashMap<_, _> = document
        .cues
        .iter()
        .enumerate()
        .map(|(i, cue)| (cue.id.as_str(), i))
        .collect();
    let mut children = vec![Vec::new(); document.cues.len()];
    let mut nested = HashSet::new();
    for (index, cue) in document.cues.iter().enumerate() {
        if music && cue.roles.iter().any(|role| role == "background") {
            if let Some(parent) = cue
                .parent_id
                .as_deref()
                .and_then(|id| indices.get(id))
                .copied()
            {
                let ancestor = &document.cues[parent];
                if cue.start_us.unwrap_or(0) >= ancestor.start_us.unwrap_or(0)
                    && ancestor
                        .end_us
                        .is_none_or(|end| cue.end_us.is_some_and(|child_end| child_end <= end))
                {
                    children[parent].push(index);
                    nested.insert(index);
                }
            }
        }
    }
    let mut pending: Vec<_> = (0..document.cues.len())
        .rev()
        .filter(|index| !nested.contains(index))
        .map(|index| (index, false, 0))
        .collect();
    while let Some((index, closing, depth)) = pending.pop() {
        if depth > 60 {
            return Err(unsupported(
                "Background nesting exceeds the TTML export depth limit.",
            ));
        }
        let cue = &document.cues[index];
        let tag = if nested.contains(&index) { "span" } else { "p" };
        if closing {
            output.push_str(&format!("</{tag}>"));
            continue;
        }
        output.push_str(&format!("<{tag}"));
        attribute(&mut output, "xml:id", &cue.id)?;
        if document.auxiliary.iter().any(|auxiliary| {
            auxiliary_supported(auxiliary, document, music) && auxiliary.target_ids[0] == cue.id
        }) {
            attribute(&mut output, "itunes:key", &cue.id)?;
        }
        if let Some(start) = cue.start_us {
            attribute(&mut output, "begin", &stamp(start))?;
        }
        if let Some(end) = cue.end_us {
            attribute(&mut output, "end", &stamp(end))?;
        }
        if let Some(language) = &cue.language {
            attribute(&mut output, "xml:lang", language)?;
        }
        if !cue.agent_ids.is_empty() {
            attribute(&mut output, "ttm:agent", &cue.agent_ids.join(" "))?;
        }
        if !cue.roles.is_empty() {
            attribute(&mut output, "ttm:role", &roles(&cue.roles))?;
        }
        output.push('>');
        write_segments(&mut output, &cue.segments, cue.start_us.unwrap_or(0), music)?;
        pending.push((index, true, depth));
        pending.extend(
            children[index]
                .iter()
                .rev()
                .map(|child| (*child, false, depth + 1)),
        );
    }
    output.push_str("</div></body></tt>");
    if !music && !document.metadata.is_empty() {
        losses.push(Diagnostic::new(
            "METADATA_DROPPED",
            "The standard TTML profile does not encode arbitrary key/value music metadata.",
            None,
        ));
    }
    if document
        .auxiliary
        .iter()
        .any(|auxiliary| !auxiliary_supported(auxiliary, document, music))
    {
        losses.push(Diagnostic::new(
            "AUXILIARY_DROPPED",
            "The selected TTML profile cannot encode this auxiliary kind or target set.",
            None,
        ));
    }
    if document
        .cues
        .iter()
        .enumerate()
        .any(|(index, cue)| cue.parent_id.is_some() && !nested.contains(&index))
    {
        losses.push(Diagnostic::new(
            "RELATIONSHIPS_DROPPED",
            "The selected profile cannot nest this cue relationship or interval.",
            None,
        ));
    }
    if crate::inspect(document)?.extensions {
        losses.push(Diagnostic::new(
            "ATTRIBUTES_DROPPED",
            "Uninterpreted canonical attributes are not emitted as executable XML extensions.",
            None,
        ));
    }
    let projected = parse(&output, music)?.document;
    if projected != *document {
        losses.push(Diagnostic::new("TTML_PROJECTION_CHANGED","TTML normalizes effective timing, inline annotations, or unsupported document structure; compare canonical JSON when exact model preservation is required.",None));
    }
    if policy == Policy::Strict && !losses.is_empty() {
        return Err(Error::new(
            "LOSSY_CONVERSION",
            serde_json::to_string(&losses).unwrap_or_default(),
        ));
    }
    Ok(ExportResult { output, losses })
}
