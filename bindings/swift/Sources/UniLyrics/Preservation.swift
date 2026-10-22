import Foundation

/// Canonical snapshot paired with exact output/format; not authenticated provenance.
public struct PreservationSidecar: Codable, Equatable, Sendable {
    public var version: UInt32
    public var format: LyricsFormat
    public var output: String
    public var document: LyricsDocument

    public init(version: UInt32 = 1, format: LyricsFormat, output: String, document: LyricsDocument) {
        self.version = version
        self.format = format
        self.output = output
        self.document = document
    }
}

/// Save both output and sidecar. Losses describe the output alone.
public struct PreservedExportResult: Codable, Equatable, Sendable {
    public var output: String
    public var losses: [LyricsDiagnostic]
    public var sidecar: PreservationSidecar

    public init(output: String, losses: [LyricsDiagnostic], sidecar: PreservationSidecar) {
        self.output = output
        self.losses = losses
        self.sidecar = sidecar
    }
}
