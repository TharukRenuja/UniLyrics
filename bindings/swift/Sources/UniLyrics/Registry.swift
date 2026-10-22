import Foundation

/// Synchronous application callbacks; captured state must be safe for concurrent use.
public struct LyricsAdapter: Sendable {
    public let matches: (@Sendable (String) -> Bool)?
    public let parse: (@Sendable (String) throws -> LyricsParseResult)?
    public let export: (@Sendable (LyricsDocument, ExportPolicy) throws -> LyricsExportResult)?

    public init(matches: (@Sendable (String) -> Bool)? = nil,
                parse: (@Sendable (String) throws -> LyricsParseResult)? = nil,
                export: (@Sendable (LyricsDocument, ExportPolicy) throws -> LyricsExportResult)? = nil) {
        self.matches = matches
        self.parse = parse
        self.export = export
    }
}

private struct CallbackRequest<Value: Encodable>: Encodable {
    let op: String
    let result: Value
    var document: LyricsDocument? = nil
    var roundtrip: LyricsDocument? = nil
    var policy: ExportPolicy? = nil
}

/// Scoped registration. The lock protects all registry state and is never held across callbacks.
public final class LyricsRegistry: @unchecked Sendable {
    private let lock = NSLock()
    private var formats: [String: LyricsAdapter] = [:]

    public init() {}

    public func register(_ name: String, adapter: LyricsAdapter) throws {
        guard (3...64).contains(name.utf8.count), name.hasPrefix("x-"),
              name.utf8.allSatisfy({ (97...122).contains($0) || (48...57).contains($0) || $0 == 45 || $0 == 95 }) else {
            throw UniLyricsError(code: "INVALID_CUSTOM_FORMAT", message: "Use a 3–64 character lowercase ASCII name in the x- namespace")
        }
        lock.lock()
        defer { lock.unlock() }
        guard formats[name] == nil else {
            throw UniLyricsError(code: "DUPLICATE_FORMAT", message: "Custom format is already registered")
        }
        formats[name] = adapter
    }

    private func adapter(_ name: String) -> LyricsAdapter? {
        lock.lock()
        defer { lock.unlock() }
        return formats[name]
    }

    private func checkInput(_ input: String) throws {
        guard input.utf8.count <= 1_048_576 else {
            throw UniLyricsError(code: "INPUT_TOO_LARGE", message: "Input exceeds 1 MiB")
        }
    }

    public func detect(_ input: String) throws -> String {
        try checkInput(input)
        var builtin: String?
        var failure: UniLyricsError?
        do { builtin = try UniLyrics.detect(input) }
        catch let error as UniLyricsError { failure = error }
        lock.lock()
        let snapshot = formats.sorted { $0.key < $1.key }
        lock.unlock()
        let matches = snapshot.filter { $0.value.matches?(input) ?? false }.map(\.key)
        if matches.count > 1 || (!matches.isEmpty && builtin != nil && builtin != "plain") {
            throw UniLyricsError(code: "AMBIGUOUS_FORMAT", message: "Multiple formats match; choose explicitly")
        }
        if let name = matches.first { return name }
        if let failure = failure { throw failure }
        guard let builtin = builtin else {
            throw UniLyricsError(code: "INVALID_RESPONSE", message: "Missing detected format")
        }
        return builtin
    }

    public func parse(_ input: String, format: String? = nil) throws -> LyricsParseResult {
        try checkInput(input)
        let name = try format ?? detect(input)
        guard let adapter = adapter(name) else {
            guard let builtin = LyricsFormat(rawValue: name) else {
                throw UniLyricsError(code: "UNKNOWN_FORMAT", message: "Format is not registered")
            }
            return try UniLyrics.parseTyped(input, format: builtin)
        }
        guard let parser = adapter.parse else {
            throw UniLyricsError(code: "UNSUPPORTED_OPERATION", message: "Custom format has no parser")
        }
        return try UniLyrics.typedCall(CallbackRequest(op: "validate_custom_parse", result: parser(input)))
    }

    public func export(_ document: LyricsDocument, format: String, policy: ExportPolicy = .compatible) throws -> LyricsExportResult {
        guard let adapter = adapter(format) else {
            guard let builtin = LyricsFormat(rawValue: format) else {
                throw UniLyricsError(code: "UNKNOWN_FORMAT", message: "Format is not registered")
            }
            return try UniLyrics.export(document, format: builtin, policy: policy)
        }
        try UniLyrics.validate(document)
        guard let exporter = adapter.export else {
            throw UniLyricsError(code: "UNSUPPORTED_OPERATION", message: "Custom format has no exporter")
        }
        let generated = try exporter(document, policy)
        try checkInput(generated.output)
        let checked: LyricsExportResult = try UniLyrics.typedCall(CallbackRequest(op: "validate_custom_export", result: generated))
        var roundtrip: LyricsDocument?
        if let parser = adapter.parse {
            do {
                let parsed: LyricsParseResult = try UniLyrics.typedCall(CallbackRequest(op: "validate_custom_parse", result: parser(checked.output)))
                roundtrip = parsed.document
            } catch { /* Failed callback reparsing is reported by shared verification. */ }
        }
        return try UniLyrics.typedCall(CallbackRequest(op: "verify_custom_export", result: checked, document: document, roundtrip: roundtrip, policy: policy))
    }
}
