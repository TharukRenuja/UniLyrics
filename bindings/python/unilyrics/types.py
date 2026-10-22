"""Canonical dictionary types; values stay ordinary, JSON-compatible dictionaries."""
from typing import Dict, List, Literal, Optional, TypedDict, Union

Json = Union[None, bool, int, float, str, List["Json"], Dict[str, "Json"]]
Attributes = Dict[str, Json]
Format = Literal["plain", "lrc", "enhanced_lrc", "json", "ttml", "ttml_lyrics", "srt", "webvtt"]
Policy = Literal["strict", "compatible"]

Profile = Literal["full", "line", "word", "karaoke", "plain", "translated", "custom"]
ProjectionFeature = Literal["line_timing", "segment_timing", "agents", "relationships", "languages", "roles", "metadata", "attributes", "auxiliary"]


class ProjectionOptions(TypedDict, total=False):
    profile: Profile
    features: List[ProjectionFeature]


class _SegmentOptions(TypedDict, total=False):
    id: str
    language: str
    agent_ids: List[str]
    roles: List[str]


class Segment(_SegmentOptions):
    text: str
    start_us: Optional[int]
    end_us: Optional[int]
    attributes: Attributes


class _CueOptions(TypedDict, total=False):
    language: str
    agent_ids: List[str]
    roles: List[str]
    parent_id: str


class Cue(_CueOptions):
    id: str
    start_us: Optional[int]
    end_us: Optional[int]
    segments: List[Segment]
    attributes: Attributes


class _AgentOptions(TypedDict, total=False):
    name: str
    kind: str


class Agent(_AgentOptions):
    id: str
    attributes: Attributes


class _AuxiliaryOptions(TypedDict, total=False):
    language: str


class Auxiliary(_AuxiliaryOptions):
    id: str
    kind: str
    target_ids: List[str]
    segments: List[Segment]
    attributes: Attributes


class Metadata(TypedDict):
    key: str
    value: str


class _DocumentOptions(TypedDict, total=False):
    language: str
    agents: List[Agent]
    auxiliary: List[Auxiliary]


class Document(_DocumentOptions):
    version: Literal[1]
    cues: List[Cue]
    metadata: List[Metadata]
    attributes: Attributes


class Features(TypedDict):
    line_timing: bool
    segment_timing: bool
    explicit_ends: bool
    agents: bool
    relationships: bool
    languages: List[str]
    roles: List[str]
    auxiliary_kinds: List[str]
    extensions: bool


class Diagnostic(TypedDict):
    code: str
    message: str
    cue_id: Optional[str]


class ParseResult(TypedDict):
    document: Document
    diagnostics: List[Diagnostic]


class ExportResult(TypedDict):
    output: str
    losses: List[Diagnostic]


class ValidationResult(TypedDict):
    valid: Literal[True]


class PreservationSidecar(TypedDict):
    version: Literal[1]
    format: Format
    output: str
    document: Document


class PreservedExportResult(TypedDict):
    output: str
    losses: List[Diagnostic]
    sidecar: PreservationSidecar
