using System.Text.Json;
using System.Text.Json.Serialization;

namespace UniLyrics;

public enum Format { Plain, Lrc, EnhancedLrc, Json, Ttml, TtmlLyrics, Srt, Webvtt }
public enum Policy { Compatible, Strict }
public enum Profile { Full, Line, Word, Karaoke, Plain, Translated, Custom }
public enum ProjectionFeature { LineTiming, SegmentTiming, Agents, Relationships, Languages, Roles, Metadata, Attributes, Auxiliary }

public sealed record Segment
{
    public required string Text { get; init; }
    public ulong? StartUs { get; init; }
    public ulong? EndUs { get; init; }
    public Dictionary<string, JsonElement> Attributes { get; init; } = new();
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Id { get; init; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Language { get; init; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public List<string>? AgentIds { get; init; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public List<string>? Roles { get; init; }
}

public sealed record Cue
{
    public required string Id { get; init; }
    public ulong? StartUs { get; init; }
    public ulong? EndUs { get; init; }
    public List<Segment> Segments { get; init; } = new();
    public Dictionary<string, JsonElement> Attributes { get; init; } = new();
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Language { get; init; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public List<string>? AgentIds { get; init; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public List<string>? Roles { get; init; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? ParentId { get; init; }
}

public sealed record Agent
{
    public required string Id { get; init; }
    public Dictionary<string, JsonElement> Attributes { get; init; } = new();
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Name { get; init; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Kind { get; init; }
}

public sealed record Auxiliary
{
    public required string Id { get; init; }
    public required string Kind { get; init; }
    public List<string> TargetIds { get; init; } = new();
    public List<Segment> Segments { get; init; } = new();
    public Dictionary<string, JsonElement> Attributes { get; init; } = new();
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Language { get; init; }
}

public sealed record Document
{
    public uint Version { get; init; } = 1;
    public List<Cue> Cues { get; init; } = new();
    public List<Metadata> Metadata { get; init; } = new();
    public Dictionary<string, JsonElement> Attributes { get; init; } = new();
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Language { get; init; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public List<Agent>? Agents { get; init; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public List<Auxiliary>? Auxiliary { get; init; }
}

public sealed record Metadata
{
    public required string Key { get; init; }
    public required string Value { get; init; }
}

public sealed record Diagnostic
{
    public required string Code { get; init; }
    public required string Message { get; init; }
    public string? CueId { get; init; }
}

public sealed record ParseResult
{
    public required Document Document { get; init; }
    public required List<Diagnostic> Diagnostics { get; init; }
}

public sealed record ExportResult
{
    public required string Output { get; init; }
    public required List<Diagnostic> Losses { get; init; }
}

public sealed record ValidationResult
{
    public bool Valid { get; init; }
}

public sealed record Features
{
    public bool LineTiming { get; init; }
    public bool SegmentTiming { get; init; }
    public bool ExplicitEnds { get; init; }
    public bool Agents { get; init; }
    public bool Relationships { get; init; }
    public bool Extensions { get; init; }
    public required List<string> Languages { get; init; }
    public required List<string> Roles { get; init; }
    public required List<string> AuxiliaryKinds { get; init; }
}

public sealed record ProjectionOptions
{
    public Profile Profile { get; init; } = Profile.Full;
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public List<ProjectionFeature>? Features { get; init; }
}

public sealed record PreservationSidecar
{
    public uint Version { get; init; } = 1;
    public Format Format { get; init; }
    public required string Output { get; init; }
    public required Document Document { get; init; }
}

public sealed record PreservedExportResult
{
    public required string Output { get; init; }
    public required List<Diagnostic> Losses { get; init; }
    public required PreservationSidecar Sidecar { get; init; }
}
