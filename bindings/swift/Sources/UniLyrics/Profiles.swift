import Foundation

public enum LyricsProfile: String, Codable, Sendable {
    case full, line, word, karaoke, plain, translated, custom
}

public enum ProjectionFeature: String, Codable, Sendable {
    case lineTiming = "line_timing"
    case segmentTiming = "segment_timing"
    case agents, relationships, languages, roles, metadata, attributes, auxiliary
}

public struct ProjectionOptions: Codable, Equatable, Sendable {
    public var profile: LyricsProfile
    public var features: [ProjectionFeature]?

    /// Custom requires an explicit list; an empty list selects text/structure only.
    public init(profile: LyricsProfile = .full, features: [ProjectionFeature]? = nil) {
        self.profile = profile
        self.features = features
    }
}
