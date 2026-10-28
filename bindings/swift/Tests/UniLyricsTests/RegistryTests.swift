import Foundation
import XCTest
@testable import UniLyrics

private enum CallbackFailure: Error { case application, changed }

private let wrappedJSON = LyricsAdapter(
    matches: { $0.hasPrefix("CUSTOM:") },
    parse: { input in
        guard input.hasPrefix("CUSTOM:") else { throw CallbackFailure.application }
        return try UniLyrics.parseTyped(String(input.dropFirst("CUSTOM:".count)), format: .json)
    },
    export: { document, policy in
        var result = try UniLyrics.export(document, format: .json, policy: policy)
        result.output = "CUSTOM:" + result.output
        return result
    })

final class RegistryTests: XCTestCase {
    private func expect(_ code: String, _ action: () throws -> Void) {
        XCTAssertThrowsError(try action()) { XCTAssertEqual(($0 as? UniLyricsError)?.code, code) }
    }

    func testSharedCorpusAndBuiltinDispatch() throws {
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { root.deleteLastPathComponent() }
        let data = try Data(contentsOf: root.appendingPathComponent("crates/unilyrics/tests/fixtures/conformance.json"))
        let cases = try XCTUnwrap(try JSONSerialization.jsonObject(with: data) as? [[String: Any]])
        let registry = LyricsRegistry()
        try registry.register("x-wrapped", adapter: wrappedJSON)
        for test in cases where test["error"] == nil {
            let input: String
            if let text = test["input"] as? String { input = text }
            else { input = String(decoding: try JSONSerialization.data(withJSONObject: test["document"]!), as: UTF8.self) }
            let format = try XCTUnwrap(test["format"] as? String)
            let direct = try UniLyrics.parseTyped(input, format: XCTUnwrap(LyricsFormat(rawValue: format)))
            XCTAssertEqual(try registry.parse(input, format: format), direct)
            let original = direct.document
            let encoded = try registry.export(original, format: "x-wrapped", policy: .strict)
            XCTAssertTrue(encoded.losses.isEmpty)
            XCTAssertEqual(try registry.detect(encoded.output), "x-wrapped")
            XCTAssertEqual(try registry.parse(encoded.output).document, original)
            XCTAssertEqual(direct.document, original)
        }
    }

    func testValidationAndAmbiguity() throws {
        let registry = LyricsRegistry()
        for name in ["plain", "x-", "X-custom", "x-💻", "x-ok\n", "x-" + String(repeating: "a", count: 63)] {
            expect("INVALID_CUSTOM_FORMAT") { try registry.register(name, adapter: wrappedJSON) }
        }
        try registry.register("x-one", adapter: wrappedJSON)
        expect("DUPLICATE_FORMAT") { try registry.register("x-one", adapter: wrappedJSON) }
        expect("UNKNOWN_FORMAT") { _ = try LyricsRegistry().parse("CUSTOM:{}", format: "x-one") }
        expect("UNKNOWN_FORMAT") { _ = try registry.parse("hi", format: "not-registered") }
        try registry.register("x-two", adapter: wrappedJSON)
        expect("AMBIGUOUS_FORMAT") { _ = try registry.detect("CUSTOM:{}") }
        let broad = LyricsRegistry()
        try broad.register("x-broad", adapter: LyricsAdapter(matches: { _ in true }))
        expect("AMBIGUOUS_FORMAT") { _ = try broad.detect("[00:01]hi") }
        XCTAssertEqual(try broad.parse("[00:01]hi", format: "lrc").document.cues[0].startUs, 1_000_000)
        expect("UNSUPPORTED_OPERATION") { _ = try broad.parse("hi", format: "x-broad") }
        expect("INPUT_TOO_LARGE") { _ = try broad.parse(String(repeating: "x", count: 1_048_577), format: "x-broad") }
        try broad.register("x-invalid", adapter: LyricsAdapter(parse: { _ in
            LyricsParseResult(document: LyricsDocument(version: 999), diagnostics: [])
        }))
        expect("UNSUPPORTED_VERSION") { _ = try broad.parse("hi", format: "x-invalid") }
    }

    func testValueSemanticsLossesAndErrors() throws {
        var original = try UniLyrics.parseTyped("සිංහල 👩🏽‍💻", format: .plain).document
        original.attributes["nested"] = .object(["value": .integer(1), "precise": .unsignedInteger(UInt64.max)])
        let before = original
        let registry = LyricsRegistry()
        try registry.register("x-mutate", adapter: LyricsAdapter(parse: wrappedJSON.parse, export: { document, policy in
            var changed = document
            changed.cues[0].segments[0].text = "changed"
            changed.attributes["nested"] = .null
            return try wrappedJSON.export!(changed, policy)
        }))
        XCTAssertEqual(try registry.export(original, format: "x-mutate").losses.map(\.code), ["CUSTOM_ROUNDTRIP_CHANGED"])
        expect("LOSSY_CONVERSION") { _ = try registry.export(original, format: "x-mutate", policy: .strict) }
        XCTAssertEqual(original, before)
        try registry.register("x-output", adapter: LyricsAdapter(export: { _, _ in LyricsExportResult(output: "opaque", losses: []) }))
        XCTAssertEqual(try registry.export(original, format: "x-output").losses.map(\.code), ["CUSTOM_ROUNDTRIP_UNVERIFIED"])
        expect("LOSSY_CONVERSION") { _ = try registry.export(original, format: "x-output", policy: .strict) }
        try registry.register("x-large", adapter: LyricsAdapter(parse: { _ in throw CallbackFailure.changed }, export: { _, _ in
            LyricsExportResult(output: String(repeating: "x", count: 1_048_577), losses: [])
        }))
        expect("INPUT_TOO_LARGE") { _ = try registry.export(original, format: "x-large") }
        try registry.register("x-reported", adapter: LyricsAdapter(parse: wrappedJSON.parse, export: { document, policy in
            var result = try wrappedJSON.export!(document, policy)
            result.losses.append(LyricsDiagnostic(code: "CUSTOM_LOSS", message: "reported"))
            return result
        }))
        expect("LOSSY_CONVERSION") { _ = try registry.export(original, format: "x-reported", policy: .strict) }
        try registry.register("x-error", adapter: LyricsAdapter(parse: { _ in throw CallbackFailure.application }, export: { _, _ in
            LyricsExportResult(output: "opaque", losses: [])
        }))
        XCTAssertThrowsError(try registry.parse("hi", format: "x-error")) { XCTAssertTrue($0 is CallbackFailure) }
        XCTAssertEqual(try registry.export(original, format: "x-error").losses.map(\.code), ["CUSTOM_ROUNDTRIP_UNVERIFIED"])
        try registry.register("x-exact", adapter: wrappedJSON)
        let encoded = try registry.export(original, format: "x-exact", policy: .strict)
        XCTAssertEqual(try registry.parse(encoded.output).document, original)
    }

    func testConcurrentAndReentrantRegistration() async throws {
        let registry = LyricsRegistry()
        try registry.register("x-wrapped", adapter: wrappedJSON)
        let original = try UniLyrics.parseTyped("hi", format: .plain).document
        let input = try wrappedJSON.export!(original, .strict).output
        try await withThrowingTaskGroup(of: Void.self) { group in
            for index in 0..<8 {
                group.addTask {
                    try registry.register("x-extra-\(index)", adapter: LyricsAdapter())
                    for _ in 0..<10 {
                        guard try registry.parse(input).document == original,
                              try registry.export(original, format: "x-wrapped", policy: .strict).losses.isEmpty else { throw CallbackFailure.changed }
                    }
                }
            }
            try await group.waitForAll()
        }
        try registry.register("x-reentrant", adapter: LyricsAdapter(parse: { [weak registry] _ in
            guard let registry = registry else { throw CallbackFailure.application }
            try registry.register("x-inside-callback", adapter: LyricsAdapter())
            return LyricsParseResult(document: original, diagnostics: [])
        }))
        XCTAssertEqual(try registry.parse("hi", format: "x-reentrant").document, original)
    }
}
