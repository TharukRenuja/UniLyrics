import Foundation

/// Arbitrary JSON attributes, retaining signed/unsigned integer precision.
public enum JSONValue: Codable, Equatable, Sendable {
    case null, bool(Bool), string(String), integer(Int64), unsignedInteger(UInt64), number(Double)
    case array([JSONValue]), object([String: JSONValue])

    public init(from decoder: Decoder) throws {
        let value = try decoder.singleValueContainer()
        if value.decodeNil() { self = .null }
        else if let v = try? value.decode(Bool.self) { self = .bool(v) }
        else if let v = try? value.decode(String.self) { self = .string(v) }
        else if let v = try? value.decode(Int64.self) { self = .integer(v) }
        else if let v = try? value.decode(UInt64.self) { self = .unsignedInteger(v) }
        else if let v = try? value.decode(Double.self) { self = .number(v) }
        else if let v = try? value.decode([JSONValue].self) { self = .array(v) }
        else { self = .object(try value.decode([String: JSONValue].self)) }
    }

    public func encode(to encoder: Encoder) throws {
        var value = encoder.singleValueContainer()
        switch self {
        case .null: try value.encodeNil()
        case .bool(let v): try value.encode(v)
        case .string(let v): try value.encode(v)
        case .integer(let v): try value.encode(v)
        case .unsignedInteger(let v): try value.encode(v)
        case .number(let v): try value.encode(v)
        case .array(let v): try value.encode(v)
        case .object(let v): try value.encode(v)
        }
    }
}

public enum LyricsFormat: String, Codable, Sendable {
    case plain, lrc, json, ttml, srt, webvtt
    case enhancedLRC = "enhanced_lrc"
    case ttmlLyrics = "ttml_lyrics"
}

public enum ExportPolicy: String, Codable, Sendable {
    case strict, compatible
}

public struct LyricsSegment: Codable, Equatable, Sendable {
    public var text: String
    public var startUs: UInt64?
    public var endUs: UInt64?
    public var attributes: [String: JSONValue]
    public var id: String?
    public var language: String?
    public var agentIds: [String]?
    public var roles: [String]?

    public init(text: String, startUs: UInt64? = nil, endUs: UInt64? = nil, attributes: [String: JSONValue] = [:], id: String? = nil, language: String? = nil, agentIds: [String]? = nil, roles: [String]? = nil) {
        self.text = text
        self.startUs = startUs
        self.endUs = endUs
        self.attributes = attributes
        self.id = id
        self.language = language
        self.agentIds = agentIds
        self.roles = roles
    }

    enum CodingKeys: String, CodingKey {
        case text
        case startUs = "start_us"
        case endUs = "end_us"
        case attributes
        case id
        case language
        case agentIds = "agent_ids"
        case roles
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(text, forKey: .text)
        try values.encode(startUs, forKey: .startUs)
        try values.encode(endUs, forKey: .endUs)
        try values.encode(attributes, forKey: .attributes)
        try values.encodeIfPresent(id, forKey: .id)
        try values.encodeIfPresent(language, forKey: .language)
        try values.encodeIfPresent(agentIds, forKey: .agentIds)
        try values.encodeIfPresent(roles, forKey: .roles)
    }
}

public struct LyricsCue: Codable, Equatable, Sendable {
    public var id: String
    public var segments: [LyricsSegment]
    public var startUs: UInt64?
    public var endUs: UInt64?
    public var attributes: [String: JSONValue]
    public var language: String?
    public var agentIds: [String]?
    public var roles: [String]?
    public var parentId: String?

    public init(id: String, segments: [LyricsSegment], startUs: UInt64? = nil, endUs: UInt64? = nil, attributes: [String: JSONValue] = [:], language: String? = nil, agentIds: [String]? = nil, roles: [String]? = nil, parentId: String? = nil) {
        self.id = id
        self.segments = segments
        self.startUs = startUs
        self.endUs = endUs
        self.attributes = attributes
        self.language = language
        self.agentIds = agentIds
        self.roles = roles
        self.parentId = parentId
    }

    enum CodingKeys: String, CodingKey {
        case id
        case segments
        case startUs = "start_us"
        case endUs = "end_us"
        case attributes
        case language
        case agentIds = "agent_ids"
        case roles
        case parentId = "parent_id"
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(id, forKey: .id)
        try values.encode(segments, forKey: .segments)
        try values.encode(startUs, forKey: .startUs)
        try values.encode(endUs, forKey: .endUs)
        try values.encode(attributes, forKey: .attributes)
        try values.encodeIfPresent(language, forKey: .language)
        try values.encodeIfPresent(agentIds, forKey: .agentIds)
        try values.encodeIfPresent(roles, forKey: .roles)
        try values.encodeIfPresent(parentId, forKey: .parentId)
    }
}

public struct LyricsAgent: Codable, Equatable, Sendable {
    public var id: String
    public var name: String?
    public var kind: String?
    public var attributes: [String: JSONValue]

    public init(id: String, name: String? = nil, kind: String? = nil, attributes: [String: JSONValue] = [:]) {
        self.id = id
        self.name = name
        self.kind = kind
        self.attributes = attributes
    }

    enum CodingKeys: String, CodingKey {
        case id
        case name
        case kind
        case attributes
    }
}

public struct LyricsAuxiliary: Codable, Equatable, Sendable {
    public var id: String
    public var kind: String
    public var segments: [LyricsSegment]
    public var targetIds: [String]
    public var language: String?
    public var attributes: [String: JSONValue]

    public init(id: String, kind: String, segments: [LyricsSegment], targetIds: [String] = [], language: String? = nil, attributes: [String: JSONValue] = [:]) {
        self.id = id
        self.kind = kind
        self.segments = segments
        self.targetIds = targetIds
        self.language = language
        self.attributes = attributes
    }

    enum CodingKeys: String, CodingKey {
        case id
        case kind
        case segments
        case targetIds = "target_ids"
        case language
        case attributes
    }
}

public struct LyricsMetadata: Codable, Equatable, Sendable {
    public var key: String
    public var value: String

    public init(key: String, value: String) {
        self.key = key
        self.value = value
    }

    enum CodingKeys: String, CodingKey {
        case key
        case value
    }
}

public struct LyricsDocument: Codable, Equatable, Sendable {
    public var version: UInt32
    public var cues: [LyricsCue]
    public var metadata: [LyricsMetadata]
    public var attributes: [String: JSONValue]
    public var language: String?
    public var agents: [LyricsAgent]?
    public var auxiliary: [LyricsAuxiliary]?

    public init(version: UInt32 = 1, cues: [LyricsCue] = [], metadata: [LyricsMetadata] = [], attributes: [String: JSONValue] = [:], language: String? = nil, agents: [LyricsAgent]? = nil, auxiliary: [LyricsAuxiliary]? = nil) {
        self.version = version
        self.cues = cues
        self.metadata = metadata
        self.attributes = attributes
        self.language = language
        self.agents = agents
        self.auxiliary = auxiliary
    }

    enum CodingKeys: String, CodingKey {
        case version
        case cues
        case metadata
        case attributes
        case language
        case agents
        case auxiliary
    }
}

public struct LyricsDiagnostic: Codable, Equatable, Sendable {
    public var code: String
    public var message: String
    public var cueId: String?

    public init(code: String, message: String, cueId: String? = nil) {
        self.code = code
        self.message = message
        self.cueId = cueId
    }

    enum CodingKeys: String, CodingKey {
        case code
        case message
        case cueId = "cue_id"
    }
}

public struct LyricsParseResult: Codable, Equatable, Sendable {
    public var document: LyricsDocument
    public var diagnostics: [LyricsDiagnostic]

    public init(document: LyricsDocument, diagnostics: [LyricsDiagnostic]) {
        self.document = document
        self.diagnostics = diagnostics
    }

    enum CodingKeys: String, CodingKey {
        case document
        case diagnostics
    }
}

public struct LyricsExportResult: Codable, Equatable, Sendable {
    public var output: String
    public var losses: [LyricsDiagnostic]

    public init(output: String, losses: [LyricsDiagnostic]) {
        self.output = output
        self.losses = losses
    }

    enum CodingKeys: String, CodingKey {
        case output
        case losses
    }
}

public struct LyricsFeatures: Codable, Equatable, Sendable {
    public var lineTiming: Bool
    public var segmentTiming: Bool
    public var explicitEnds: Bool
    public var agents: Bool
    public var relationships: Bool
    public var languages: [String]
    public var roles: [String]
    public var auxiliaryKinds: [String]
    public var extensions: Bool

    public init(lineTiming: Bool, segmentTiming: Bool, explicitEnds: Bool, agents: Bool, relationships: Bool, languages: [String], roles: [String], auxiliaryKinds: [String], extensions: Bool) {
        self.lineTiming = lineTiming
        self.segmentTiming = segmentTiming
        self.explicitEnds = explicitEnds
        self.agents = agents
        self.relationships = relationships
        self.languages = languages
        self.roles = roles
        self.auxiliaryKinds = auxiliaryKinds
        self.extensions = extensions
    }

    enum CodingKeys: String, CodingKey {
        case lineTiming = "line_timing"
        case segmentTiming = "segment_timing"
        case explicitEnds = "explicit_ends"
        case agents
        case relationships
        case languages
        case roles
        case auxiliaryKinds = "auxiliary_kinds"
        case extensions
    }
}
