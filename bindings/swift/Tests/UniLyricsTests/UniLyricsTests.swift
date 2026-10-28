import XCTest
import Foundation
@testable import UniLyrics

final class UniLyricsTests: XCTestCase {
    func testSharedFixtures() throws {
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { root.deleteLastPathComponent() }
        let data = try Data(contentsOf: root.appendingPathComponent("crates/unilyrics/tests/fixtures/conformance.json"))
        let cases = try XCTUnwrap(try JSONSerialization.jsonObject(with: data) as? [[String: Any]])
        for test in cases {
            let input: String
            if let text = test["input"] as? String { input = text }
            else { input = String(decoding: try JSONSerialization.data(withJSONObject: test["document"]!), as: UTF8.self) }
            let format = try XCTUnwrap(test["format"] as? String)
            if let code = test["error"] as? String {
                XCTAssertThrowsError(try UniLyrics.parse(input, format: format)) { error in
                    XCTAssertEqual((error as? UniLyricsError)?.code, code)
                }
            } else {
                let result = try XCTUnwrap(try UniLyrics.parse(input, format: format) as? [String: Any])
                let document = try XCTUnwrap(result["document"] as? [String: Any])
                let typed = try UniLyrics.parseTyped(input, format: XCTUnwrap(LyricsFormat(rawValue: format)))
                try UniLyrics.validate(typed.document)
                let typedJSON = try UniLyrics.export(typed.document, format: .json, policy: .strict)
                XCTAssertEqual(try UniLyrics.parseTyped(typedJSON.output, format: .json).document, typed.document)
                let typedObject = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(typedJSON.output.utf8)) as? NSDictionary)
                XCTAssertEqual(typedObject, document as NSDictionary)
                XCTAssertEqual(typed.document.cues.map { $0.segments.map(\.text).joined() }, test["text"] as? [String])
                XCTAssertEqual(typed.document.cues.map(\.startUs), (test["starts"] as? [Any])?.map { ($0 as? NSNumber)?.uint64Value })

                for expected in test["exports"] as? [[String:Any]] ?? [] {
                    let format = try XCTUnwrap(expected["format"] as? String)
                    let policy = try XCTUnwrap(expected["policy"] as? String)
                    let encoded = try XCTUnwrap(try UniLyrics.export(document, format:format, policy:policy) as? [String:Any])
                    XCTAssertEqual(encoded["output"] as? String, expected["output"] as? String)
                    let typedExport = try UniLyrics.export(typed.document, format: XCTUnwrap(LyricsFormat(rawValue: format)), policy: XCTUnwrap(ExportPolicy(rawValue: policy)))
                    XCTAssertEqual(typedExport.output, expected["output"] as? String)
                    XCTAssertTrue(typedExport.losses.isEmpty)
                    XCTAssertEqual((encoded["losses"] as? [Any])?.count, 0)
                }
                if let features = test["features"] {
                    let actual = try UniLyrics.inspect(document)
                    XCTAssertEqual(try JSONSerialization.data(withJSONObject: actual, options:.sortedKeys), try JSONSerialization.data(withJSONObject: features, options:.sortedKeys))
                    XCTAssertEqual(try JSONSerialization.data(withJSONObject: document, options:.sortedKeys), try JSONSerialization.data(withJSONObject: test["document"]!, options:.sortedKeys))
                    try UniLyrics.validate(document)
                    let exported = try XCTUnwrap(try UniLyrics.export(document, format:"json", policy:"strict") as? [String:Any])
                    let reparsed = try XCTUnwrap(try UniLyrics.parse(try XCTUnwrap(exported["output"] as? String), format:"json") as? [String:Any])
                    XCTAssertEqual(try JSONSerialization.data(withJSONObject: reparsed["document"]!, options:.sortedKeys), try JSONSerialization.data(withJSONObject: document, options:.sortedKeys))
                    let plain = try XCTUnwrap(try UniLyrics.export(document, format:"plain") as? [String:Any])
                    let losses = try XCTUnwrap(plain["losses"] as? [[String:Any]])
                    XCTAssertEqual(Set(losses.compactMap { $0["code"] as? String }), Set(try XCTUnwrap(test["loss_codes"] as? [String])))
                    XCTAssertThrowsError(try UniLyrics.export(document, format:"plain", policy:"strict")) { error in
                        XCTAssertEqual((error as? UniLyricsError)?.code, "LOSSY_CONVERSION")
                    }
                }
                let cues = try XCTUnwrap(document["cues"] as? [[String: Any]])
                let text = try cues.map { cue in
                    try XCTUnwrap(cue["segments"] as? [[String: Any]]).map { try XCTUnwrap($0["text"] as? String) }.joined()
                }
                XCTAssertEqual(text, test["text"] as? [String])
                let starts = cues.map { $0["start_us"] ?? NSNull() }
                XCTAssertEqual(try JSONSerialization.data(withJSONObject: starts), try JSONSerialization.data(withJSONObject: test["starts"]!))
            }
        }
    }

    func testNormalizationFixtures() throws {
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { root.deleteLastPathComponent() }
        let data = try Data(contentsOf: root.appendingPathComponent("crates/unilyrics/tests/fixtures/normalization.json"))
        let cases = try XCTUnwrap(try JSONSerialization.jsonObject(with: data) as? [[String: Any]])
        for test in cases {
            let raw = try XCTUnwrap(test["document"] as? [String: Any])
            let document = try JSONDecoder().decode(LyricsDocument.self, from: JSONSerialization.data(withJSONObject: raw))
            let before = document
            let offset = (test["offset_us"] as? NSNumber)?.int64Value ?? 0
            if let error = test["error"] as? String {
                XCTAssertThrowsError(try UniLyrics.normalize(document, offsetUs: offset)) {
                    XCTAssertEqual(($0 as? UniLyricsError)?.code, error)
                }
                XCTAssertThrowsError(try UniLyrics.normalize(raw, offsetUs: offset)) {
                    XCTAssertEqual(($0 as? UniLyricsError)?.code, error)
                }
            } else {
                let expected = try JSONDecoder().decode(LyricsDocument.self, from: JSONSerialization.data(withJSONObject: test["expected_document"]!))
                let result = try UniLyrics.normalize(document, offsetUs: offset)
                XCTAssertEqual(result.document, expected)
                XCTAssertEqual(result.diagnostics.map(\.code), test["diagnostic_codes"] as? [String])
                let rawResult = try XCTUnwrap(try UniLyrics.normalize(raw, offsetUs: offset) as? [String: Any])
                let rawDocument = try XCTUnwrap(rawResult["document"] as? NSDictionary)
                XCTAssertEqual(rawDocument, test["expected_document"] as? NSDictionary)
            }
            XCTAssertEqual(document, before)
        }
    }

    func testTypedAuthoringAndErrors() throws {
        let attributes: [String: JSONValue] = [
            "signed": .integer(Int64.min), "unsigned": .unsignedInteger(UInt64.max),
            "precise": .integer(9_007_199_254_740_993), "fraction": .number(0.125),
            "nested": .array([.null, .bool(true), .object(["text": .string("සිංහල 👩🏽‍💻")])])
        ]
        let document = LyricsDocument(
            cues: [LyricsCue(id: "cue", segments: [LyricsSegment(text: "Hello", startUs: 0, endUs: 500_000, id: "word", language: "en", agentIds: ["singer"], roles: ["lead"])], startUs: 0, endUs: 1_000_000)],
            metadata: [LyricsMetadata(key: "title", value: "Song")], attributes: attributes,
            language: "en", agents: [LyricsAgent(id: "singer", name: "Voice", kind: "person")],
            auxiliary: [LyricsAuxiliary(id: "translation", kind: "translation", segments: [LyricsSegment(text: "ආයුබෝවන්")], targetIds: ["word"], language: "si")])
        try UniLyrics.validate(document)
        let features = try UniLyrics.inspect(document)
        XCTAssertTrue(features.agents)
        XCTAssertTrue(features.segmentTiming)
        XCTAssertEqual(features.auxiliaryKinds, ["translation"])
        let encoded = try UniLyrics.export(document, format: .json, policy: .strict)
        XCTAssertEqual(try UniLyrics.parseTyped(encoded.output, format: .json).document, document)
        XCTAssertEqual(try UniLyrics.detectTyped("[00:01]hi"), .lrc)
        XCTAssertThrowsError(try UniLyrics.export(document, format: .plain, policy: .strict)) {
            XCTAssertEqual(($0 as? UniLyricsError)?.code, "LOSSY_CONVERSION")
        }
        XCTAssertThrowsError(try UniLyrics.parseTyped("[00:60]bad", format: .lrc)) {
            XCTAssertEqual(($0 as? UniLyricsError)?.code, "INVALID_TIMESTAMP")
        }
        var invalid = document
        invalid.cues[0].startUs = 9_007_199_254_740_992
        XCTAssertThrowsError(try UniLyrics.validate(invalid))
        invalid = document
        invalid.attributes["nan"] = .number(.nan)
        XCTAssertThrowsError(try UniLyrics.export(invalid, format: .json))
    }

    func testProfileHelperDefaultsAndErrors() throws {
        let document = try UniLyrics.parseTyped("[00:01]hello", format: .lrc).document
        XCTAssertEqual(try UniLyrics.project(document).document, document)
        let options = ProjectionOptions(profile: .custom, features: [])
        let projected = try UniLyrics.project(document, options: options)
        XCTAssertNil(projected.document.cues[0].startUs)
        XCTAssertFalse(projected.diagnostics.isEmpty)
        for invalid in [ProjectionOptions(profile: .custom),
                        ProjectionOptions(profile: .full, features: []),
                        ProjectionOptions(profile: .custom, features: [.agents, .agents])] {
            XCTAssertThrowsError(try UniLyrics.project(document, options: invalid))
        }
        XCTAssertThrowsError(try UniLyrics.exportProfile(document, format: .json, options: options, policy: .strict)) {
            XCTAssertEqual(($0 as? UniLyricsError)?.code, "LOSSY_CONVERSION")
        }
        XCTAssertEqual(document.cues[0].startUs, 1_000_000)
    }

    func testProfileProtocolFixtures() throws {
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { root.deleteLastPathComponent() }
        let data = try Data(contentsOf: root.appendingPathComponent("crates/unilyrics/tests/fixtures/profiles.json"))
        let cases = try XCTUnwrap(try JSONSerialization.jsonObject(with: data) as? [[String: Any]])
        for test in cases {
            let request = try XCTUnwrap(test["request"] as? [String: Any])
            if let code = test["error"] as? String {
                XCTAssertThrowsError(try UniLyrics.process(request)) { XCTAssertEqual(($0 as? UniLyricsError)?.code, code) }
            } else {
                let result = try XCTUnwrap(try UniLyrics.process(request) as? [String: Any])
                let rawDocument = try XCTUnwrap(request["document"] as? [String: Any])
                let document = try JSONDecoder().decode(LyricsDocument.self, from: JSONSerialization.data(withJSONObject: rawDocument))
                let rawOptions = request["options"] as? [String: Any] ?? [:]
                var options = ProjectionOptions()
                if !rawOptions.isEmpty {
                    options = try JSONDecoder().decode(ProjectionOptions.self, from: JSONSerialization.data(withJSONObject: rawOptions))
                }
                let helperData: Data
                let dictionaryHelper: Any
                if request["op"] as? String == "project" {
                    helperData = try JSONEncoder().encode(UniLyrics.project(document, options: options))
                    dictionaryHelper = try UniLyrics.project(rawDocument, options: rawOptions)
                } else {
                    let formatName = try XCTUnwrap(request["format"] as? String)
                    let format = try XCTUnwrap(LyricsFormat(rawValue: formatName))
                    let policyName = request["policy"] as? String ?? "compatible"
                    let policy = try XCTUnwrap(ExportPolicy(rawValue: policyName))
                    helperData = try JSONEncoder().encode(UniLyrics.exportProfile(document, format: format, options: options, policy: policy))
                    dictionaryHelper = try UniLyrics.exportProfile(rawDocument, format: formatName, options: rawOptions, policy: policyName)
                }
                let protocolData = try JSONSerialization.data(withJSONObject: result)
                if request["op"] as? String == "project" {
                    XCTAssertEqual(try JSONDecoder().decode(LyricsParseResult.self, from: helperData),
                                   try JSONDecoder().decode(LyricsParseResult.self, from: protocolData))
                } else {
                    XCTAssertEqual(try JSONDecoder().decode(LyricsExportResult.self, from: helperData),
                                   try JSONDecoder().decode(LyricsExportResult.self, from: protocolData))
                }
                XCTAssertEqual(dictionaryHelper as? NSDictionary, result as NSDictionary)
                let actual: NSDictionary
                let expected: NSDictionary
                let diagnostics: [[String: Any]]
                if let document = test["expected_document"] as? NSDictionary {
                    expected = document
                    actual = try XCTUnwrap(result["document"] as? NSDictionary)
                    diagnostics = try XCTUnwrap(result["diagnostics"] as? [[String: Any]])
                } else {
                    expected = try XCTUnwrap(test["output_document"] as? NSDictionary)
                    let output = try XCTUnwrap(result["output"] as? String)
                    actual = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(output.utf8)) as? NSDictionary)
                    diagnostics = try XCTUnwrap(result["losses"] as? [[String: Any]])
                }
                XCTAssertEqual(actual, expected)
                XCTAssertEqual(diagnostics.compactMap { $0["code"] as? String }, test["codes"] as? [String])
            }
        }
    }

    func testNativeParsingAndErrors() throws {
        let result = try XCTUnwrap(try UniLyrics.parse("[00:01.25]你好", format: "lrc") as? [String: Any])
        let document = try XCTUnwrap(result["document"] as? [String: Any])
        try UniLyrics.validate(document)
        XCTAssertEqual(try UniLyrics.detect("[00:01]hi"), "lrc")
        let cues = try XCTUnwrap(document["cues"] as? [[String: Any]])
        XCTAssertEqual(cues.first?["start_us"] as? Int, 1_250_000)
        XCTAssertThrowsError(try UniLyrics.parse("[00:60]bad", format: "lrc")) { error in
            XCTAssertEqual((error as? UniLyricsError)?.code, "INVALID_TIMESTAMP")
        }
    }
}
