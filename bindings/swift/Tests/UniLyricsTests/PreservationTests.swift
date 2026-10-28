import Foundation
import XCTest
@testable import UniLyrics

final class PreservationTests: XCTestCase {
    func testRichProfileSnapshotsAndDictionaryHelpers() throws {
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { root.deleteLastPathComponent() }
        let data = try Data(contentsOf: root.appendingPathComponent("crates/unilyrics/tests/fixtures/profiles.json"))
        let cases = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [[String: Any]])
        for test in cases {
            let request = try XCTUnwrap(test["request"] as? [String: Any])
            if test["error"] != nil || request["op"] as? String != "project" { continue }
            let rawDocument = try XCTUnwrap(request["document"] as? [String: Any])
            let document = try JSONDecoder().decode(LyricsDocument.self, from: JSONSerialization.data(withJSONObject: rawDocument))
            let rawOptions = request["options"] as? [String: Any] ?? [:]
            let options = rawOptions.isEmpty ? ProjectionOptions() : try JSONDecoder().decode(ProjectionOptions.self, from: JSONSerialization.data(withJSONObject: rawOptions))
            let bundle = try UniLyrics.exportPreserved(document, format: .json, options: options)
            let expected = try XCTUnwrap(test["expected_document"] as? NSDictionary)
            XCTAssertEqual(try JSONSerialization.jsonObject(with: Data(bundle.output.utf8)) as? NSDictionary, expected)
            XCTAssertEqual(bundle.losses.map(\.code), test["codes"] as? [String])
            let stored = try JSONEncoder().encode(bundle.sidecar)
            let reloaded = try JSONDecoder().decode(PreservationSidecar.self, from: stored)
            XCTAssertEqual(try UniLyrics.restorePreserved(bundle.output, format: .json, sidecar: reloaded).document, document)
            XCTAssertEqual(bundle.sidecar.document, document)
            let rawBundle = try XCTUnwrap(UniLyrics.exportPreserved(rawDocument, format: "json", options: rawOptions) as? [String: Any])
            let decoded = try JSONDecoder().decode(PreservedExportResult.self, from: JSONSerialization.data(withJSONObject: rawBundle))
            XCTAssertEqual(decoded, bundle)
            let rawSidecar = try XCTUnwrap(rawBundle["sidecar"] as? [String: Any])
            let restored = try XCTUnwrap(UniLyrics.restorePreserved(bundle.output, format: "json", sidecar: rawSidecar) as? [String: Any])
            let typedRestored = try JSONDecoder().decode(LyricsParseResult.self, from: JSONSerialization.data(withJSONObject: restored))
            XCTAssertEqual(typedRestored.document, document)
            XCTAssertThrowsError(try UniLyrics.restorePreserved(bundle.output + "\n", format: .json, sidecar: reloaded)) {
                XCTAssertEqual(($0 as? UniLyricsError)?.code, "SIDECAR_MISMATCH")
            }
            XCTAssertThrowsError(try UniLyrics.restorePreserved(bundle.output, format: .plain, sidecar: reloaded)) {
                XCTAssertEqual(($0 as? UniLyricsError)?.code, "SIDECAR_MISMATCH")
            }
            var invalid = reloaded
            invalid.version = 2
            XCTAssertThrowsError(try UniLyrics.restorePreserved(bundle.output, format: .json, sidecar: invalid)) {
                XCTAssertEqual(($0 as? UniLyricsError)?.code, "UNSUPPORTED_SIDECAR_VERSION")
            }
        }
    }

    func testEncodedRestoreBudget() throws {
        let document = try UniLyrics.parseTyped(String(repeating: "/", count: 200_000), format: .plain).document
        XCTAssertThrowsError(try UniLyrics.exportPreserved(document, format: .plain)) {
            XCTAssertEqual(($0 as? UniLyricsError)?.code, "INPUT_TOO_LARGE")
        }
        let raw = try XCTUnwrap(try UniLyrics.parse(String(repeating: "/", count: 200_000), format: "plain") as? [String: Any])
        let rawDocument = try XCTUnwrap(raw["document"] as? [String: Any])
        XCTAssertThrowsError(try UniLyrics.exportPreserved(rawDocument, format: "plain")) {
            XCTAssertEqual(($0 as? UniLyricsError)?.code, "INPUT_TOO_LARGE")
        }
        let smaller = try UniLyrics.parseTyped(String(repeating: "/", count: 100_000), format: .plain).document
        let bundle = try UniLyrics.exportPreserved(smaller, format: .plain)
        XCTAssertEqual(try UniLyrics.restorePreserved(bundle.output, format: .plain, sidecar: bundle.sidecar).document, smaller)
    }

    func testLossyPlainDefaultsAndInvalidSnapshot() throws {
        let document = try UniLyrics.parseTyped("[00:01]සිංහල 👩🏽‍💻", format: .lrc).document
        let bundle = try UniLyrics.exportPreserved(document, format: .plain)
        XCTAssertFalse(bundle.losses.isEmpty)
        let restored = try UniLyrics.restorePreserved(bundle.output, format: .plain, sidecar: bundle.sidecar)
        XCTAssertEqual(restored.document, document)
        XCTAssertEqual(restored.diagnostics.map(\.code), ["SIDECAR_RESTORED"])
        var invalid = bundle.sidecar
        invalid.document.version = 99
        XCTAssertThrowsError(try UniLyrics.restorePreserved(bundle.output, format: .plain, sidecar: invalid))
        XCTAssertEqual(document.cues[0].startUs, 1_000_000)
    }
}
