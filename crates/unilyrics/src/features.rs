use crate::{validate, Document, Error, Features};
use std::collections::BTreeSet;

/// Inspect source facts without inferring words, singers, languages, or missing timing.
pub fn inspect(document: &Document) -> Result<Features, Error> {
    validate(document)?;
    let segments = || {
        document
            .cues
            .iter()
            .flat_map(|cue| &cue.segments)
            .chain(document.auxiliary.iter().flat_map(|aux| &aux.segments))
    };
    let languages: BTreeSet<_> = document
        .language
        .iter()
        .chain(document.cues.iter().filter_map(|cue| cue.language.as_ref()))
        .chain(
            document
                .auxiliary
                .iter()
                .filter_map(|aux| aux.language.as_ref()),
        )
        .chain(segments().filter_map(|segment| segment.language.as_ref()))
        .cloned()
        .collect();
    let roles: BTreeSet<_> = document
        .cues
        .iter()
        .flat_map(|cue| &cue.roles)
        .chain(segments().flat_map(|segment| &segment.roles))
        .cloned()
        .collect();
    let kinds: BTreeSet<_> = document
        .auxiliary
        .iter()
        .map(|aux| aux.kind.clone())
        .collect();
    Ok(Features {
        line_timing: document
            .cues
            .iter()
            .any(|cue| cue.start_us.is_some() || cue.end_us.is_some()),
        segment_timing: segments()
            .any(|segment| segment.start_us.is_some() || segment.end_us.is_some()),
        explicit_ends: document.cues.iter().any(|cue| cue.end_us.is_some())
            || segments().any(|segment| segment.end_us.is_some()),
        agents: !document.agents.is_empty(),
        relationships: document.cues.iter().any(|cue| cue.parent_id.is_some())
            || document
                .auxiliary
                .iter()
                .any(|aux| !aux.target_ids.is_empty()),
        languages: languages.into_iter().collect(),
        roles: roles.into_iter().collect(),
        auxiliary_kinds: kinds.into_iter().collect(),
        extensions: !document.attributes.is_empty()
            || document.cues.iter().any(|cue| !cue.attributes.is_empty())
            || document
                .agents
                .iter()
                .any(|agent| !agent.attributes.is_empty())
            || document
                .auxiliary
                .iter()
                .any(|aux| !aux.attributes.is_empty())
            || segments().any(|segment| !segment.attributes.is_empty()),
    })
}
