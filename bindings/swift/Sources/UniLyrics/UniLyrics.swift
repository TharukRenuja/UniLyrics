import Foundation
import CUniLyrics

public struct UniLyricsError: Error {
    public let code: String
    public let message: String
    public init(code: String, message: String) {
        self.code = code
        self.message = message
    }
}

public enum UniLyrics {
    private static func checkRequestSize(_ data: Data) throws {
        guard data.count <= 1_048_576 else {
            throw UniLyricsError(code: "INPUT_TOO_LARGE", message: "Request exceeds 1 MiB")
        }
    }

    private static func processData(_ data: Data) throws -> Data {
        try checkRequestSize(data)
        let output = data.withUnsafeBytes { bytes in
            unilyrics_process(bytes.bindMemory(to: UInt8.self).baseAddress, data.count)
        }
        guard let output = output else {
            throw UniLyricsError(code: "INVALID_RESPONSE", message: "No response from native core")
        }
        defer { unilyrics_free(output) }
        return Data(String(cString: output).utf8)
    }

    /// Returns ordinary Foundation JSON values, throwing structured core errors.
    public static func process(_ request: [String: Any]) throws -> Any {
        let response = try JSONSerialization.jsonObject(with: processData(JSONSerialization.data(withJSONObject: request)))
        guard let object = response as? [String: Any], let ok = object["ok"] as? Bool else {
            throw UniLyricsError(code: "INVALID_RESPONSE", message: "Invalid response from native core")
        }
        if !ok {
            let error = object["error"] as? [String: Any] ?? [:]
            throw UniLyricsError(code: error["code"] as? String ?? "UNKNOWN", message: error["message"] as? String ?? "Unknown core error")
        }
        guard let result = object["result"] else {
            throw UniLyricsError(code: "INVALID_RESPONSE", message: "Missing result")
        }
        return result
    }

    public static func parse(_ input: String, format: String? = nil) throws -> Any {
        var request: [String: Any] = ["op": "parse", "input": input]
        if let format = format { request["format"] = format }
        return try process(request)
    }

    public static func export(_ document: [String: Any], format: String, policy: String = "compatible") throws -> Any {
        try process(["op": "export", "document": document, "format": format, "policy": policy])
    }

    public static func detect(_ input: String) throws -> String {
        guard let format = try process(["op": "detect", "input": input]) as? String else {
            throw UniLyricsError(code: "INVALID_RESPONSE", message: "Expected a format name")
        }
        return format
    }

    public static func inspect(_ document: [String: Any]) throws -> [String: Any] {
        guard let features = try process(["op": "inspect", "document": document]) as? [String: Any] else {
            throw UniLyricsError(code: "INVALID_RESPONSE", message: "Expected feature information")
        }
        return features
    }

    public static func validate(_ document: [String: Any]) throws {
        _ = try process(["op": "validate", "document": document])
    }
}


extension UniLyrics {
    private struct TypedRequest: Encodable {
        let op: String
        var input: String? = nil
        var document: LyricsDocument? = nil
        var format: LyricsFormat? = nil
        var policy: ExportPolicy? = nil
        var offset_us: Int64? = nil
        var options: ProjectionOptions? = nil
        var sidecar: PreservationSidecar? = nil
    }

    private struct CoreError: Decodable {
        let code: String
        let message: String
    }

    private struct Reply<Result: Decodable>: Decodable {
        let ok: Bool
        let result: Result?
        let error: CoreError?
    }

    static func typedCall<Result: Decodable, Request: Encodable>(_ request: Request) throws -> Result {
        let data = try processData(JSONEncoder().encode(request))
        let reply = try JSONDecoder().decode(Reply<Result>.self, from: data)
        if !reply.ok {
            guard let error = reply.error else {
                throw UniLyricsError(code: "INVALID_RESPONSE", message: "Missing core error")
            }
            throw UniLyricsError(code: error.code, message: error.message)
        }
        guard let result = reply.result else {
            throw UniLyricsError(code: "INVALID_RESPONSE", message: "Missing result")
        }
        return result
    }

    public static func parseTyped(_ input: String, format: LyricsFormat? = nil) throws -> LyricsParseResult {
        try typedCall(TypedRequest(op: "parse", input: input, format: format))
    }

    public static func detectTyped(_ input: String) throws -> LyricsFormat {
        try typedCall(TypedRequest(op: "detect", input: input))
    }

    public static func export(_ document: LyricsDocument, format: LyricsFormat, policy: ExportPolicy = .compatible) throws -> LyricsExportResult {
        try typedCall(TypedRequest(op: "export", document: document, format: format, policy: policy))
    }

    public static func inspect(_ document: LyricsDocument) throws -> LyricsFeatures {
        try typedCall(TypedRequest(op: "inspect", document: document))
    }

    public static func validate(_ document: LyricsDocument) throws {
        struct Valid: Decodable { let valid: Bool }
        let result: Valid = try typedCall(TypedRequest(op: "validate", document: document))
        guard result.valid else {
            throw UniLyricsError(code: "INVALID_RESPONSE", message: "Expected successful validation")
        }
    }

    public static func exportPreserved(_ document: LyricsDocument, format: LyricsFormat, options: ProjectionOptions = .init()) throws -> PreservedExportResult {
        let result: PreservedExportResult = try typedCall(TypedRequest(op: "export_preserved", document: document, format: format, options: options))
        try checkRequestSize(JSONEncoder().encode(TypedRequest(op: "restore_preserved", input: result.output, format: format, sidecar: result.sidecar)))
        return result
    }

    public static func restorePreserved(_ input: String, format: LyricsFormat, sidecar: PreservationSidecar) throws -> LyricsParseResult {
        try typedCall(TypedRequest(op: "restore_preserved", input: input, format: format, sidecar: sidecar))
    }

    public static func exportPreserved(_ document: [String: Any], format: String, options: [String: Any] = [:]) throws -> Any {
        let result = try process(["op": "export_preserved", "document": document, "format": format, "options": options])
        guard let bundle = result as? [String: Any], let output = bundle["output"] as? String, let sidecar = bundle["sidecar"] as? [String: Any] else {
            throw UniLyricsError(code: "INVALID_RESPONSE", message: "Expected a preservation bundle")
        }
        try checkRequestSize(JSONSerialization.data(withJSONObject: ["op": "restore_preserved", "input": output, "format": format, "sidecar": sidecar]))
        return result
    }

    public static func restorePreserved(_ input: String, format: String, sidecar: [String: Any]) throws -> Any {
        try process(["op": "restore_preserved", "input": input, "format": format, "sidecar": sidecar])
    }

    public static func project(_ document: LyricsDocument, options: ProjectionOptions = .init()) throws -> LyricsParseResult {
        try typedCall(TypedRequest(op: "project", document: document, options: options))
    }

    public static func exportProfile(_ document: LyricsDocument, format: LyricsFormat, options: ProjectionOptions = .init(), policy: ExportPolicy = .compatible) throws -> LyricsExportResult {
        try typedCall(TypedRequest(op: "export_profile", document: document, format: format, policy: policy, options: options))
    }

    public static func project(_ document: [String: Any], options: [String: Any] = [:]) throws -> Any {
        try process(["op": "project", "document": document, "options": options])
    }

    public static func exportProfile(_ document: [String: Any], format: String, options: [String: Any] = [:], policy: String = "compatible") throws -> Any {
        try process(["op": "export_profile", "document": document, "format": format, "options": options, "policy": policy])
    }

    public static func normalize(_ document: LyricsDocument, offsetUs: Int64 = 0) throws -> LyricsParseResult {
        try typedCall(TypedRequest(op: "normalize", document: document, offset_us: offsetUs))
    }

    public static func normalize(_ document: [String: Any], offsetUs: Int64 = 0) throws -> Any {
        try process(["op": "normalize", "document": document, "offset_us": offsetUs])
    }
}
