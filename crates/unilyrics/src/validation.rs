use crate::{Document, Error, Segment, MAX_SAFE_TIME_US};
use std::collections::{HashMap, HashSet};

fn interval(start: Option<u64>, end: Option<u64>) -> Result<(), Error> {
    if [start, end]
        .into_iter()
        .flatten()
        .any(|t| t > MAX_SAFE_TIME_US)
    {
        return Err(Error::new(
            "INVALID_TIME",
            "Time is outside the JSON-safe microsecond range.",
        ));
    }
    if matches!((start, end), (Some(start), Some(end)) if end < start) {
        return Err(Error::new("INVALID_INTERVAL", "End precedes start."));
    }
    Ok(())
}

fn id<'a>(value: &'a str, ids: &mut HashSet<&'a str>) -> Result<(), Error> {
    if value.is_empty() || !ids.insert(value) {
        return Err(Error::new(
            "INVALID_ID",
            "Document IDs must be nonempty and globally unique.",
        ));
    }
    Ok(())
}

fn language(value: Option<&str>) -> Result<(), Error> {
    if value.is_some_and(str::is_empty) {
        return Err(Error::new(
            "INVALID_LANGUAGE",
            "A declared language must not be empty.",
        ));
    }
    Ok(())
}

fn annotations(
    language_tag: Option<&str>,
    roles: &[String],
    references: &[String],
    agents: &HashSet<&str>,
) -> Result<(), Error> {
    language(language_tag)?;
    if roles.iter().any(String::is_empty) {
        return Err(Error::new("INVALID_ROLE", "Roles must not be empty."));
    }
    if references
        .iter()
        .any(|agent| !agents.contains(agent.as_str()))
    {
        return Err(Error::new(
            "UNKNOWN_AGENT",
            "A vocal reference does not name a declared agent.",
        ));
    }
    Ok(())
}

fn segments<'a>(
    values: &'a [Segment],
    agents: &HashSet<&str>,
    ids: &mut HashSet<&'a str>,
) -> Result<(), Error> {
    for segment in values {
        if let Some(value) = &segment.id {
            id(value, ids)?;
        }
        interval(segment.start_us, segment.end_us)?;
        annotations(
            segment.language.as_deref(),
            &segment.roles,
            &segment.agent_ids,
            agents,
        )?;
    }
    Ok(())
}

pub fn validate(document: &Document) -> Result<(), Error> {
    if document.version != 1 {
        return Err(Error::new(
            "UNSUPPORTED_VERSION",
            "Only document version 1 is accepted.",
        ));
    }
    if document.cues.len() > 10_000
        || document.agents.len() > 10_000
        || document.auxiliary.len() > 10_000
    {
        return Err(Error::new(
            "DOCUMENT_TOO_LARGE",
            "At most 10000 cues, agents, and auxiliary entries are accepted.",
        ));
    }
    let segment_count: usize = document
        .cues
        .iter()
        .map(|cue| cue.segments.len())
        .chain(document.auxiliary.iter().map(|aux| aux.segments.len()))
        .sum();
    let reference_count: usize = document
        .cues
        .iter()
        .map(|cue| cue.agent_ids.len() + cue.roles.len())
        .chain(document.auxiliary.iter().map(|aux| aux.target_ids.len()))
        .chain(
            document
                .cues
                .iter()
                .flat_map(|cue| &cue.segments)
                .chain(document.auxiliary.iter().flat_map(|aux| &aux.segments))
                .map(|segment| segment.agent_ids.len() + segment.roles.len()),
        )
        .sum();
    if segment_count > 100_000 || reference_count > 100_000 {
        return Err(Error::new(
            "DOCUMENT_TOO_LARGE",
            "At most 100000 segments and 100000 annotation references are accepted.",
        ));
    }
    language(document.language.as_deref())?;
    let agents: HashSet<&str> = document
        .agents
        .iter()
        .map(|agent| agent.id.as_str())
        .collect();
    let mut ids = HashSet::new();
    for agent in &document.agents {
        id(&agent.id, &mut ids)?;
        if agent.kind.as_deref().is_some_and(str::is_empty) {
            return Err(Error::new(
                "INVALID_AGENT",
                "A declared agent kind must not be empty.",
            ));
        }
    }
    let cue_indices: HashMap<&str, usize> = document
        .cues
        .iter()
        .enumerate()
        .map(|(index, cue)| (cue.id.as_str(), index))
        .collect();
    let mut targets = HashSet::new();
    for cue in &document.cues {
        id(&cue.id, &mut ids)?;
        targets.insert(cue.id.as_str());
        interval(cue.start_us, cue.end_us)?;
        annotations(cue.language.as_deref(), &cue.roles, &cue.agent_ids, &agents)?;
        segments(&cue.segments, &agents, &mut ids)?;
        targets.extend(
            cue.segments
                .iter()
                .filter_map(|segment| segment.id.as_deref()),
        );
        if cue
            .parent_id
            .as_deref()
            .is_some_and(|parent| !cue_indices.contains_key(parent))
        {
            return Err(Error::new(
                "UNKNOWN_PARENT",
                "Cue parent does not name a cue.",
            ));
        }
    }
    // Resolve each parent chain once, without recursive calls on untrusted content.
    let mut state = vec![0; document.cues.len()];
    for start in 0..document.cues.len() {
        let mut current = Some(start);
        let mut path = Vec::new();
        while let Some(index) = current {
            if state[index] == 2 {
                break;
            }
            if state[index] == 1 {
                return Err(Error::new(
                    "CYCLIC_RELATIONSHIP",
                    "Cue parent relationships must not form a cycle.",
                ));
            }
            state[index] = 1;
            path.push(index);
            current = document.cues[index]
                .parent_id
                .as_deref()
                .map(|parent| cue_indices[parent]);
        }
        for index in path {
            state[index] = 2;
        }
    }
    for aux in &document.auxiliary {
        id(&aux.id, &mut ids)?;
        if aux.kind.is_empty() {
            return Err(Error::new(
                "INVALID_AUXILIARY",
                "Auxiliary content needs a nonempty kind.",
            ));
        }
        language(aux.language.as_deref())?;
        if aux
            .target_ids
            .iter()
            .any(|target| !targets.contains(target.as_str()))
        {
            return Err(Error::new(
                "UNKNOWN_TARGET",
                "Auxiliary target does not name a cue or a primary-content segment.",
            ));
        }
        segments(&aux.segments, &agents, &mut ids)?;
    }
    Ok(())
}
