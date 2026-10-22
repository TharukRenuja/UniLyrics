use crate::{
    export, inspect, validate, Diagnostic, Document, Error, ExportResult, Format, ParseResult,
    Policy,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    #[default]
    Full,
    Line,
    Word,
    Karaoke,
    Plain,
    Translated,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionFeature {
    LineTiming,
    SegmentTiming,
    Agents,
    Relationships,
    Languages,
    Roles,
    Metadata,
    Attributes,
    Auxiliary,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectionOptions {
    pub profile: Profile,
    pub features: Option<Vec<ProjectionFeature>>,
}

impl ProjectionOptions {
    fn selection(&self) -> Result<BTreeSet<ProjectionFeature>, Error> {
        use ProjectionFeature::*;
        if self.profile == Profile::Custom {
            let features = self.features.as_ref().ok_or_else(|| {
                Error::new(
                    "INVALID_PROFILE",
                    "Custom profile requires an explicit features list.",
                )
            })?;
            let selected: BTreeSet<_> = features.iter().copied().collect();
            if selected.len() != features.len() {
                return Err(Error::new(
                    "INVALID_PROFILE",
                    "Duplicate custom features are not allowed.",
                ));
            }
            return Ok(selected);
        }
        if self.features.is_some() {
            return Err(Error::new(
                "INVALID_PROFILE",
                "Feature lists apply only to the custom profile.",
            ));
        }
        Ok(match self.profile {
            Profile::Full => vec![
                LineTiming,
                SegmentTiming,
                Agents,
                Relationships,
                Languages,
                Roles,
                Metadata,
                Attributes,
                Auxiliary,
            ],
            Profile::Line => vec![LineTiming, Languages, Metadata],
            Profile::Word => vec![LineTiming, SegmentTiming, Languages, Metadata],
            Profile::Karaoke => vec![
                LineTiming,
                SegmentTiming,
                Agents,
                Relationships,
                Languages,
                Roles,
                Metadata,
            ],
            Profile::Plain => vec![],
            Profile::Translated => vec![
                LineTiming,
                SegmentTiming,
                Agents,
                Relationships,
                Languages,
                Roles,
                Metadata,
                Auxiliary,
            ],
            Profile::Custom => unreachable!(),
        }
        .into_iter()
        .collect())
    }
}

/// Project canonical features without changing primary text, identities, order or segmentation.
pub fn project(document: &Document, options: &ProjectionOptions) -> Result<ParseResult, Error> {
    validate(document)?;
    let selection = options.selection()?;
    let keep = |feature| selection.contains(&feature);
    use ProjectionFeature::*;
    let before = inspect(document)?;
    let mut result = ParseResult {
        document: document.clone(),
        diagnostics: vec![],
    };
    let mut loss = |code, message| {
        result
            .diagnostics
            .push(Diagnostic::new(code, message, None))
    };
    if !keep(LineTiming) && before.line_timing {
        loss(
            "PROFILE_LINE_TIMING_DROPPED",
            "Profile removes explicit cue timing.",
        );
    }
    if !keep(SegmentTiming) && before.segment_timing {
        loss(
            "PROFILE_SEGMENT_TIMING_DROPPED",
            "Profile removes explicit segment and auxiliary timing.",
        );
    }
    if !keep(Agents) && before.agents {
        loss(
            "PROFILE_AGENTS_DROPPED",
            "Profile removes agent declarations and references.",
        );
    }
    if !keep(Relationships) && before.relationships {
        loss(
            "PROFILE_RELATIONSHIPS_DROPPED",
            "Profile removes parent and auxiliary target relationships.",
        );
    }
    if !keep(Languages) && !before.languages.is_empty() {
        loss(
            "PROFILE_LANGUAGES_DROPPED",
            "Profile removes language annotations.",
        );
    }
    if !keep(Roles) && !before.roles.is_empty() {
        loss("PROFILE_ROLES_DROPPED", "Profile removes role annotations.");
    }
    if !keep(Metadata) && !document.metadata.is_empty() {
        loss(
            "PROFILE_METADATA_DROPPED",
            "Profile removes ordered metadata.",
        );
    }
    if !keep(Attributes) && before.extensions {
        loss(
            "PROFILE_ATTRIBUTES_DROPPED",
            "Profile removes opaque attributes.",
        );
    }
    let target = &mut result.document;
    if !keep(Auxiliary) {
        target.auxiliary.clear();
    } else if options.profile == Profile::Translated {
        target.auxiliary.retain(|entry| entry.kind == "translation");
    }
    if target.auxiliary.len() != document.auxiliary.len() {
        loss(
            "PROFILE_AUXILIARY_DROPPED",
            "Profile removes auxiliary content outside its selected scope.",
        );
    }
    if !keep(Metadata) {
        target.metadata.clear();
    }
    if !keep(Attributes) {
        target.attributes.clear();
    }
    if !keep(Languages) {
        target.language = None;
    }
    if !keep(Agents) {
        target.agents.clear();
    } else if !keep(Attributes) {
        for agent in &mut target.agents {
            agent.attributes.clear();
        }
    }
    for cue in &mut target.cues {
        if !keep(LineTiming) {
            cue.start_us = None;
            cue.end_us = None;
        }
        if !keep(Relationships) {
            cue.parent_id = None;
        }
        if !keep(Languages) {
            cue.language = None;
        }
        if !keep(Roles) {
            cue.roles.clear();
        }
        if !keep(Agents) {
            cue.agent_ids.clear();
        }
        if !keep(Attributes) {
            cue.attributes.clear();
        }
    }
    for auxiliary in &mut target.auxiliary {
        if !keep(Relationships) {
            auxiliary.target_ids.clear();
        }
        if !keep(Languages) {
            auxiliary.language = None;
        }
        if !keep(Attributes) {
            auxiliary.attributes.clear();
        }
    }
    for segment in target
        .cues
        .iter_mut()
        .flat_map(|cue| &mut cue.segments)
        .chain(
            target
                .auxiliary
                .iter_mut()
                .flat_map(|entry| &mut entry.segments),
        )
    {
        if !keep(SegmentTiming) {
            segment.start_us = None;
            segment.end_us = None;
        }
        if !keep(Languages) {
            segment.language = None;
        }
        if !keep(Roles) {
            segment.roles.clear();
        }
        if !keep(Agents) {
            segment.agent_ids.clear();
        }
        if !keep(Attributes) {
            segment.attributes.clear();
        }
    }
    validate(target)?;
    if keep(Attributes) && before.extensions && target != document {
        result.diagnostics.push(Diagnostic::new("PROFILE_EXTENSIONS_UNCHANGED", "Opaque attributes were retained verbatim and may require application-specific updates after projection.", None));
    }
    Ok(result)
}

/// Aggregate intentional profile losses with target-format losses; strict means no feature loss.
pub fn export_profile(
    document: &Document,
    format: Format,
    options: &ProjectionOptions,
    policy: Policy,
) -> Result<ExportResult, Error> {
    let mut projected = project(document, options)?;
    if policy == Policy::Strict && !projected.diagnostics.is_empty() {
        return Err(Error::new(
            "LOSSY_CONVERSION",
            serde_json::to_string(&projected.diagnostics)
                .map_err(|e| Error::new("SERIALIZATION_ERROR", e.to_string()))?,
        ));
    }
    let mut result = export(&projected.document, format, policy)?;
    projected.diagnostics.append(&mut result.losses);
    result.losses = projected.diagnostics;
    Ok(result)
}
