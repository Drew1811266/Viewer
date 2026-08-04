import Foundation
import Darwin

private let protocolVersion = 1
private let allowedCommands: Set<String> = [
    "inspect",
    "query",
    "activate",
    "focus",
    "setValue",
    "key",
    "pointer",
    "drag",
    "capture",
    "shutdown",
]

private func writeResponse(_ object: [String: Any]) {
    do {
        let data = try JSONSerialization.data(withJSONObject: object, options: [])
        FileHandle.standardOutput.write(data)
        FileHandle.standardOutput.write(Data([0x0A]))
    } catch {
        let fallback = "{\"version\":1,\"sequence\":0,\"ok\":false,\"error\":{\"code\":\"SAFETY_PROTOCOL\",\"message\":\"Response encoding failed\"}}\n"
        FileHandle.standardOutput.write(Data(fallback.utf8))
    }
}

private func errorResponse(
    sequence: Int,
    code: String,
    message: String
) -> [String: Any] {
    [
        "version": protocolVersion,
        "sequence": sequence,
        "ok": false,
        "error": ["code": code, "message": message],
    ]
}

private func handleProtocolTestLine(_ line: String) -> [String: Any] {
    guard
        let data = line.data(using: .utf8),
        let object = try? JSONSerialization.jsonObject(with: data),
        let request = object as? [String: Any]
    else {
        return errorResponse(
            sequence: 0,
            code: "SAFETY_PROTOCOL",
            message: "Malformed JSON request"
        )
    }

    let sequence = request["sequence"] as? Int ?? 0
    guard let command = request["command"] as? String, allowedCommands.contains(command) else {
        return errorResponse(
            sequence: sequence,
            code: "SAFETY_COMMAND",
            message: "Unsupported command"
        )
    }

    let expectedKeys: Set<String> = [
        "version", "sequence", "pid", "windowId", "command", "timeoutMs", "payload",
    ]
    guard
        Set(request.keys) == expectedKeys,
        request["version"] as? Int == protocolVersion,
        sequence > 0,
        let pid = request["pid"] as? Int,
        pid > 0,
        let windowID = request["windowId"] as? Int,
        windowID > 0,
        let timeout = request["timeoutMs"] as? Int,
        (100 ... 10_000).contains(timeout),
        request["payload"] is [String: Any]
    else {
        return errorResponse(
            sequence: sequence,
            code: "SAFETY_PROTOCOL",
            message: "Invalid protocol envelope"
        )
    }

    return [
        "version": protocolVersion,
        "sequence": sequence,
        "ok": true,
        "result": ["mode": "protocol-test"],
    ]
}

if !CommandLine.arguments.dropFirst().contains("--protocol-test") {
    FileHandle.standardError.write(
        Data("Production native adapter is not available yet\n".utf8)
    )
    exit(64)
}

while let line = readLine() {
    writeResponse(handleProtocolTestLine(line))
}
