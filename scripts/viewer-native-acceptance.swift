import Foundation
import Darwin
import AppKit
import ApplicationServices
import CoreGraphics
import CryptoKit
import ImageIO
import ScreenCaptureKit
import UniformTypeIdentifiers

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
    "finderDrag",
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

private struct AcceptanceFailure: Error {
    let code: String
    let message: String
}

private struct RequestEnvelope {
    let sequence: Int
    let pid: Int
    let windowID: Int
    let command: String
    let timeoutMilliseconds: Int
    let payload: [String: Any]
}

private protocol NativeAdapter {
    func handle(_ request: RequestEnvelope) throws -> [String: Any]
}

private struct ProtocolTestAdapter: NativeAdapter {
    func handle(_: RequestEnvelope) throws -> [String: Any] {
        ["mode": "protocol-test"]
    }
}

private struct FixtureElement {
    let role: String
    let name: String
    let identifier: String
    let enabled: Bool
    let focused: Bool
    let frame: [String: Int]
    let path: [Int]

    var dictionary: [String: Any] {
        [
            "role": role,
            "name": name,
            "identifier": identifier,
            "enabled": enabled,
            "focused": focused,
            "frame": frame,
            "path": path,
        ]
    }
}

private struct FixtureAdapter: NativeAdapter {
    private let pid = 101
    private let windowID = 44
    private let frame = ["x": 20, "y": 30, "width": 1024, "height": 720]
    private let elements = [
        FixtureElement(
            role: "AXButton",
            name: "筛选",
            identifier: "toolbar-filter",
            enabled: true,
            focused: false,
            frame: ["x": 900, "y": 40, "width": 80, "height": 32],
            path: [0, 0]
        ),
        FixtureElement(
            role: "AXButton",
            name: "重复",
            identifier: "duplicate-one",
            enabled: true,
            focused: false,
            frame: ["x": 700, "y": 40, "width": 80, "height": 32],
            path: [0, 1]
        ),
        FixtureElement(
            role: "AXButton",
            name: "重复",
            identifier: "duplicate-two",
            enabled: true,
            focused: false,
            frame: ["x": 790, "y": 40, "width": 80, "height": 32],
            path: [0, 2]
        ),
        FixtureElement(
            role: "AXTextField",
            name: "搜索",
            identifier: "toolbar-search",
            enabled: true,
            focused: false,
            frame: ["x": 120, "y": 40, "width": 400, "height": 32],
            path: [0, 3]
        ),
    ]

    func handle(_ request: RequestEnvelope) throws -> [String: Any] {
        guard request.pid == pid, request.windowID == windowID else {
            throw AcceptanceFailure(
                code: "PRECONDITION_WINDOW_OWNER",
                message: "Target window changed"
            )
        }

        switch request.command {
        case "inspect":
            return [
                "pid": pid,
                "windowId": windowID,
                "title": "Viewer Acceptance Fixture",
                "frame": frame,
                "focused": true,
                "frontmost": true,
            ]
        case "query":
            return try query(request.payload)
        case "activate", "focus", "setValue":
            _ = try resolveUniqueElement(request.payload)
            return ["performed": true, "command": request.command]
        case "pointer":
            try validatePointer(request.payload)
            return ["performed": true, "command": request.command]
        case "drag":
            guard
                let from = request.payload["from"] as? [String: Any],
                let to = request.payload["to"] as? [String: Any]
            else {
                throw AcceptanceFailure(
                    code: "SAFETY_COMMAND",
                    message: "Invalid drag payload"
                )
            }
            try validatePoint(from)
            try validatePoint(to)
            return ["performed": true, "command": request.command]
        case "finderDrag":
            guard let destination = request.payload["destination"] as? [String: Any] else {
                throw AcceptanceFailure(
                    code: "SAFETY_COMMAND",
                    message: "Invalid Finder drag payload"
                )
            }
            try validatePoint(destination)
            return ["performed": true, "command": request.command]
        default:
            return ["performed": true, "command": request.command]
        }
    }

    private func query(_ payload: [String: Any]) throws -> [String: Any] {
        ["elements": [try resolveUniqueElement(payload).dictionary]]
    }

    private func resolveUniqueElement(_ payload: [String: Any]) throws -> FixtureElement {
        guard
            Set(payload.keys).contains("target"),
            let target = payload["target"] as? [String: Any],
            !target.isEmpty,
            Set(target.keys).isSubset(of: ["role", "name", "namePrefix", "identifier", "position"]),
            target["position"] == nil || target["position"] as? String == "rightmost"
        else {
            throw AcceptanceFailure(
                code: "SAFETY_COMMAND",
                message: "Invalid query target"
            )
        }

        var matches = elements.filter { element in
            if let role = target["role"] as? String, element.role != role { return false }
            if let name = target["name"] as? String, element.name != name { return false }
            if let namePrefix = target["namePrefix"] as? String,
               !element.name.hasPrefix(namePrefix)
            {
                return false
            }
            if let identifier = target["identifier"] as? String,
               element.identifier != identifier
            {
                return false
            }
            return true
        }
        if target["position"] as? String == "rightmost", let rightmost = matches.max(
            by: { ($0.frame["x"] ?? 0) < ($1.frame["x"] ?? 0) }
        ) {
            matches = [rightmost]
        }
        guard !matches.isEmpty else {
            throw AcceptanceFailure(
                code: "STATE_TARGET_NOT_FOUND",
                message: "Target element was not found"
            )
        }
        guard matches.count == 1 else {
            throw AcceptanceFailure(
                code: "STATE_TARGET_NOT_UNIQUE",
                message: "Target element was not unique"
            )
        }
        return matches[0]
    }

    private func validatePointer(_ payload: [String: Any]) throws {
        guard let point = payload["point"] as? [String: Any] else {
            throw AcceptanceFailure(
                code: "SAFETY_COMMAND",
                message: "Invalid pointer payload"
            )
        }
        try validatePoint(point)
    }

    private func validatePoint(_ point: [String: Any]) throws {
        guard let x = numberValue(point["x"]),
              let y = numberValue(point["y"]),
              x >= 0,
              y >= 0,
              x < 1024,
              y < 720
        else {
            throw AcceptanceFailure(
                code: "SAFETY_POINT_OUTSIDE_WINDOW",
                message: "Pointer coordinate is outside the approved window"
            )
        }
    }
}

private let allowedKeys: Set<String> = [
    "tab", "enter", "space", "escape", "arrowUp", "arrowDown", "arrowLeft",
    "arrowRight", "home", "period", "end", "delete", "backspace", "f10",
    "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m",
    "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z",
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9",
]
private let allowedModifiers: Set<String> = ["shift", "control", "option", "command"]

private func numberValue(_ value: Any?) -> Double? {
    if let number = value as? NSNumber { return number.doubleValue }
    return nil
}

private func validateTargetPayload(_ target: Any?) throws {
    guard
        let target = target as? [String: Any],
        !target.isEmpty,
        Set(target.keys).isSubset(of: ["role", "name", "namePrefix", "identifier", "position"]),
        target["position"] == nil || target["position"] as? String == "rightmost"
    else {
        throw AcceptanceFailure(code: "SAFETY_COMMAND", message: "Invalid target selector")
    }
    for value in target.values {
        guard let text = value as? String, !text.isEmpty, text.count <= 256 else {
            throw AcceptanceFailure(code: "SAFETY_COMMAND", message: "Invalid target selector")
        }
    }
}

private func validatePointPayload(_ value: Any?) throws {
    guard
        let point = value as? [String: Any],
        Set(point.keys) == ["x", "y"],
        numberValue(point["x"])?.isFinite == true,
        numberValue(point["y"])?.isFinite == true
    else {
        throw AcceptanceFailure(code: "SAFETY_COMMAND", message: "Invalid pointer coordinate")
    }
}

private func validateRequestPayload(_ request: RequestEnvelope) throws {
    let payload = request.payload
    switch request.command {
    case "inspect", "shutdown":
        guard payload.isEmpty else {
            throw AcceptanceFailure(code: "SAFETY_COMMAND", message: "Unexpected payload fields")
        }
    case "query", "activate", "focus":
        guard Set(payload.keys) == ["target"] else {
            throw AcceptanceFailure(code: "SAFETY_COMMAND", message: "Invalid target command")
        }
        try validateTargetPayload(payload["target"])
    case "setValue":
        guard Set(payload.keys) == ["target", "text"],
              let text = payload["text"] as? String,
              text.count <= 4096
        else {
            throw AcceptanceFailure(code: "SAFETY_COMMAND", message: "Invalid setValue payload")
        }
        try validateTargetPayload(payload["target"])
    case "key":
        guard Set(payload.keys) == ["key", "modifiers"],
              let key = payload["key"] as? String,
              allowedKeys.contains(key),
              let modifiers = payload["modifiers"] as? [String],
              Set(modifiers).count == modifiers.count,
              Set(modifiers).isSubset(of: allowedModifiers)
        else {
            throw AcceptanceFailure(code: "SAFETY_COMMAND", message: "Invalid key payload")
        }
    case "pointer":
        guard let kind = payload["kind"] as? String else {
            throw AcceptanceFailure(code: "SAFETY_COMMAND", message: "Invalid pointer payload")
        }
        if kind == "scroll" {
            guard Set(payload.keys) == ["kind", "point", "deltaY"],
                  let delta = payload["deltaY"] as? Int,
                  delta != 0,
                  abs(delta) <= 1000
            else {
                throw AcceptanceFailure(code: "SAFETY_COMMAND", message: "Invalid scroll payload")
            }
        } else {
            let fields = Set(payload.keys)
            let modifiers = payload["modifiers"] as? [String] ?? []
            guard fields == ["kind", "point"] || fields == ["kind", "point", "modifiers"],
                  [
                      "click", "doubleClick", "rightClick", "move",
                      "leftDown", "leftDrag", "leftUp",
                      "rightDown", "rightDrag", "rightUp",
                  ].contains(kind),
                  !["move", "leftUp", "rightUp"].contains(kind) || modifiers.isEmpty,
                  Set(modifiers).count == modifiers.count,
                  Set(modifiers).isSubset(of: allowedModifiers)
            else {
                throw AcceptanceFailure(code: "SAFETY_COMMAND", message: "Invalid pointer payload")
            }
        }
        try validatePointPayload(payload["point"])
    case "drag":
        guard Set(payload.keys) == ["from", "to", "durationMs"],
              let duration = payload["durationMs"] as? Int,
              (50 ... 5000).contains(duration)
        else {
            throw AcceptanceFailure(code: "SAFETY_COMMAND", message: "Invalid drag payload")
        }
        try validatePointPayload(payload["from"])
        try validatePointPayload(payload["to"])
    case "finderDrag":
        guard Set(payload.keys) == ["path", "destination", "release", "durationMs"],
              let sourcePath = payload["path"] as? String,
              (sourcePath as NSString).isAbsolutePath,
              !sourcePath.contains(where: { "$~*?[]{}".contains($0) }),
              payload["release"] is Bool,
              let duration = payload["durationMs"] as? Int,
              (50 ... 5000).contains(duration)
        else {
            throw AcceptanceFailure(code: "SAFETY_COMMAND", message: "Invalid Finder drag payload")
        }
        let fixtureRoot = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("ViewerAcceptanceRuns", isDirectory: true)
            .standardizedFileURL.path
        let standardizedSource = URL(fileURLWithPath: sourcePath).standardizedFileURL.path
        guard standardizedSource == fixtureRoot || standardizedSource.hasPrefix(fixtureRoot + "/") else {
            throw AcceptanceFailure(
                code: "SAFETY_COMMAND",
                message: "Finder drag path escapes the approved fixture root"
            )
        }
        try validatePointPayload(payload["destination"])
    case "capture":
        guard Set(payload.keys) == ["path"],
              let capturePath = payload["path"] as? String,
              (capturePath as NSString).isAbsolutePath,
              capturePath.hasSuffix(".png")
        else {
            throw AcceptanceFailure(code: "SAFETY_COMMAND", message: "Invalid capture path")
        }
    default:
        throw AcceptanceFailure(code: "SAFETY_COMMAND", message: "Unsupported command")
    }
}

private struct MacWindowSnapshot {
    let windowID: Int
    let title: String
    let frame: CGRect
    let frontmost: Bool

    var dictionary: [String: Any] {
        [
            "windowId": windowID,
            "title": title,
            "frame": [
                "x": Int(frame.origin.x.rounded()),
                "y": Int(frame.origin.y.rounded()),
                "width": Int(frame.width.rounded()),
                "height": Int(frame.height.rounded()),
            ],
            "focused": frontmost,
            "frontmost": frontmost,
        ]
    }
}

private protocol MacSystem {
    func validateProcess(_ pid: Int) throws
    func accessibilityTrusted() -> Bool
    func windowSnapshot(pid: Int, windowID: Int) throws -> MacWindowSnapshot
    func query(
        pid: Int,
        window: MacWindowSnapshot,
        target: [String: Any]
    ) throws -> [[String: Any]]
    func perform(
        command: String,
        payload: [String: Any],
        pid: Int,
        window: MacWindowSnapshot,
        timeoutMilliseconds: Int
    ) throws -> [String: Any]
    func capture(
        path: String,
        pid: Int,
        window: MacWindowSnapshot
    ) throws -> [String: Any]
}

private struct MacAdapter: NativeAdapter {
    let system: any MacSystem

    func handle(_ request: RequestEnvelope) throws -> [String: Any] {
        try system.validateProcess(request.pid)
        guard system.accessibilityTrusted() else {
            throw AcceptanceFailure(
                code: "PRECONDITION_ACCESSIBILITY",
                message: "Enable Accessibility for the acceptance helper in System Settings > Privacy & Security > Accessibility"
            )
        }
        let window = try system.windowSnapshot(pid: request.pid, windowID: request.windowID)

        switch request.command {
        case "inspect":
            var result = window.dictionary
            result["pid"] = request.pid
            return result
        case "query":
            return [
                "elements": try system.query(
                    pid: request.pid,
                    window: window,
                    target: request.payload["target"] as? [String: Any] ?? [:]
                ),
            ]
        case "capture":
            guard let path = request.payload["path"] as? String else {
                throw AcceptanceFailure(code: "SAFETY_COMMAND", message: "Invalid capture path")
            }
            return try system.capture(path: path, pid: request.pid, window: window)
        case "shutdown":
            return ["performed": true, "command": request.command]
        default:
            return try system.perform(
                command: request.command,
                payload: request.payload,
                pid: request.pid,
                window: window,
                timeoutMilliseconds: request.timeoutMilliseconds
            )
        }
    }
}

private final class RecordingMacSystem: MacSystem {
    private let frame = CGRect(x: 20, y: 30, width: 1024, height: 720)
    private var isFrontmost: Bool

    init(frontmost: Bool = true) {
        isFrontmost = frontmost
    }

    func validateProcess(_ pid: Int) throws {
        guard pid == 101 else {
            throw AcceptanceFailure(
                code: "PRECONDITION_VIEWER_PID",
                message: "Viewer PID is not running"
            )
        }
    }

    func accessibilityTrusted() -> Bool { true }

    func windowSnapshot(pid _: Int, windowID: Int) throws -> MacWindowSnapshot {
        guard windowID == 44 else {
            throw AcceptanceFailure(
                code: "PRECONDITION_WINDOW_OWNER",
                message: "Target window changed"
            )
        }
        return MacWindowSnapshot(
            windowID: 44,
            title: "Viewer Mac Adapter Fixture",
            frame: frame,
            frontmost: isFrontmost
        )
    }

    func query(
        pid _: Int,
        window _: MacWindowSnapshot,
        target: [String: Any]
    ) throws -> [[String: Any]] {
        guard target["identifier"] as? String == "toolbar-filter" else {
            throw AcceptanceFailure(
                code: "STATE_TARGET_NOT_FOUND",
                message: "Target element was not found"
            )
        }
        return [[
            "role": "AXButton",
            "name": "筛选",
            "identifier": "toolbar-filter",
            "enabled": true,
            "focused": false,
            "frame": ["x": 900, "y": 40, "width": 80, "height": 32],
            "path": [0, 0],
        ]]
    }

    func perform(
        command: String,
        payload: [String: Any],
        pid _: Int,
        window _: MacWindowSnapshot,
        timeoutMilliseconds _: Int
    ) throws -> [String: Any] {
        if command == "focus" { isFrontmost = true }
        if command == "finderDrag" {
            return [
                "performed": true,
                "command": command,
                "held": !(payload["release"] as? Bool ?? true),
            ]
        }
        return ["performed": true, "command": command]
    }

    func capture(
        path _: String,
        pid _: Int,
        window _: MacWindowSnapshot
    ) throws -> [String: Any] {
        [
            "performed": true,
            "command": "capture",
            "width": 2048,
            "height": 1440,
            "sha256": String(repeating: "a", count: 64),
        ]
    }
}

private struct AXMatch {
    let element: AXUIElement
    let dictionary: [String: Any]
}

private final class LiveMacSystem: MacSystem {
    private var externalDragHeld = false
    private var heldFinderWindow: AXUIElement?
    private var heldFinderWindowFrame: CGRect?

    func validateProcess(_ pid: Int) throws {
        errno = 0
        if kill(pid_t(pid), 0) == -1, errno == ESRCH {
            throw AcceptanceFailure(
                code: "PRECONDITION_VIEWER_PID",
                message: "Viewer PID is not running"
            )
        }
    }

    func accessibilityTrusted() -> Bool { AXIsProcessTrusted() }

    func windowSnapshot(pid: Int, windowID: Int) throws -> MacWindowSnapshot {
        guard
            let rawWindows = CGWindowListCopyWindowInfo(
                [.optionOnScreenOnly, .excludeDesktopElements],
                kCGNullWindowID
            ) as? [[String: Any]]
        else {
            throw AcceptanceFailure(
                code: "PRECONDITION_WINDOW_OWNER",
                message: "Target window changed"
            )
        }

        let matches = rawWindows.filter { item in
            let ownerPID = item[kCGWindowOwnerPID as String] as? Int
            let number = item[kCGWindowNumber as String] as? Int
            let layer = item[kCGWindowLayer as String] as? Int
            return ownerPID == pid && number == windowID && layer == 0
        }
        guard matches.count == 1,
              let bounds = matches[0][kCGWindowBounds as String] as? [String: Any],
              let frame = CGRect(dictionaryRepresentation: bounds as CFDictionary)
        else {
            throw AcceptanceFailure(
                code: "PRECONDITION_WINDOW_OWNER",
                message: "Target window changed"
            )
        }

        let frontmostPID = NSWorkspace.shared.frontmostApplication?.processIdentifier
        return MacWindowSnapshot(
            windowID: windowID,
            title: matches[0][kCGWindowName as String] as? String ?? "Viewer",
            frame: frame,
            frontmost: frontmostPID == pid_t(pid)
        )
    }

    func query(
        pid: Int,
        window: MacWindowSnapshot,
        target: [String: Any]
    ) throws -> [[String: Any]] {
        [try uniqueAXMatch(pid: pid, window: window, target: target).dictionary]
    }

    func perform(
        command: String,
        payload: [String: Any],
        pid: Int,
        window: MacWindowSnapshot,
        timeoutMilliseconds: Int
    ) throws -> [String: Any] {
        if command == "focus" {
            try focusTarget(
                payload: payload,
                pid: pid,
                window: window,
                timeoutMilliseconds: timeoutMilliseconds
            )
            return ["performed": true, "command": command]
        }
        if command == "finderDrag" {
            return try postFinderDrag(payload, pid: pid, window: window)
        }
        if command == "pointer",
           payload["kind"] as? String == "leftUp",
           externalDragHeld
        {
            try requireStableWindow(pid: pid, window: window, requireFrontmost: false)
            try postPointer(payload, pid: pid, window: window)
            externalDragHeld = false
            restoreHeldFinderWindow()
            return ["performed": true, "command": command]
        }
        try requireStableFrontmostWindow(pid: pid, window: window)
        switch command {
        case "activate":
            let match = try uniqueAXMatch(
                pid: pid,
                window: window,
                target: payload["target"] as? [String: Any] ?? [:]
            )
            guard AXUIElementPerformAction(match.element, kAXPressAction as CFString) == .success else {
                throw AcceptanceFailure(
                    code: "STATE_ACTION_FAILED",
                    message: "Accessibility press action failed"
                )
            }
        case "setValue":
            let match = try uniqueAXMatch(
                pid: pid,
                window: window,
                target: payload["target"] as? [String: Any] ?? [:]
            )
            let role = match.dictionary["role"] as? String
            guard ["AXTextField", "AXTextArea", "AXComboBox"].contains(role),
                  let text = payload["text"] as? String,
                  AXUIElementSetAttributeValue(
                      match.element,
                      kAXValueAttribute as CFString,
                      text as CFString
                  ) == .success
            else {
                throw AcceptanceFailure(
                    code: "STATE_ACTION_FAILED",
                    message: "Accessibility value action failed"
                )
            }
        case "key":
            try postKey(payload)
        case "pointer":
            try postPointer(payload, pid: pid, window: window)
        case "drag":
            try postDrag(payload, window: window)
        default:
            throw AcceptanceFailure(
                code: "SAFETY_COMMAND",
                message: "Unsupported native action"
            )
        }
        return ["performed": true, "command": command]
    }

    private func postFinderDrag(
        _ payload: [String: Any],
        pid: Int,
        window: MacWindowSnapshot
    ) throws -> [String: Any] {
        guard !externalDragHeld,
              let sourcePath = payload["path"] as? String,
              let release = payload["release"] as? Bool,
              let duration = payload["durationMs"] as? Int
        else {
            throw AcceptanceFailure(
                code: "STATE_ACTION_FAILED",
                message: "A Finder drag is already active or malformed"
            )
        }

        let fixtureRoot = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("ViewerAcceptanceRuns", isDirectory: true)
            .standardizedFileURL
            .resolvingSymlinksInPath()
        let literalSource = URL(fileURLWithPath: sourcePath).standardizedFileURL
        let resolvedSource = literalSource.resolvingSymlinksInPath()
        guard literalSource.path == resolvedSource.path,
              resolvedSource.path.hasPrefix(fixtureRoot.path + "/"),
              FileManager.default.fileExists(atPath: resolvedSource.path)
        else {
            throw AcceptanceFailure(
                code: "SAFETY_COMMAND",
                message: "Finder drag source is outside the exact disposable fixture"
            )
        }

        let destination = try screenPoint(payload["destination"], window: window)
        NSWorkspace.shared.activateFileViewerSelecting([resolvedSource])
        let finder = try waitForFinder(timeoutMilliseconds: 3000)
        let finderWindow = try focusedFinderWindow(pid: finder.processIdentifier)
        let originalFrame = axFrame(finderWindow)
        if let stagedFrame = stagedFinderFrame(around: window.frame),
           !setAXFrame(finderWindow, frame: stagedFrame)
        {
            throw AcceptanceFailure(
                code: "STATE_ACTION_FAILED",
                message: "Unable to stage the Finder source window"
            )
        }

        let currentFinderFrame = axFrame(finderWindow) ?? originalFrame ?? .zero
        let sourceElement = try waitForFinderSelection(
            pid: finder.processIdentifier,
            name: resolvedSource.lastPathComponent,
            windowFrame: currentFinderFrame,
            timeoutMilliseconds: 3000
        )
        let sourceFrame = axFrame(sourceElement) ?? .zero
        guard sourceFrame.width >= 4, sourceFrame.height >= 4 else {
            if let originalFrame { _ = setAXFrame(finderWindow, frame: originalFrame) }
            throw AcceptanceFailure(
                code: "STATE_TARGET_NOT_FOUND",
                message: "Finder selected item has no usable frame"
            )
        }
        let source = CGPoint(x: sourceFrame.midX, y: sourceFrame.midY)
        var mouseIsDown = false
        do {
            try postMouse(type: .mouseMoved, point: source)
            try postMouse(type: .leftMouseDown, point: source)
            mouseIsDown = true
            usleep(100_000)

            let steps = max(12, min(60, duration / 20))
            for index in 1 ... steps {
                let progress = CGFloat(index) / CGFloat(steps)
                let point = CGPoint(
                    x: source.x + ((destination.x - source.x) * progress),
                    y: source.y + ((destination.y - source.y) * progress)
                )
                try postMouse(type: .leftMouseDragged, point: point)
                usleep(useconds_t(max(1, duration * 1000 / steps)))
            }

            if release {
                try postMouse(type: .leftMouseUp, point: destination)
                mouseIsDown = false
                if let originalFrame { _ = setAXFrame(finderWindow, frame: originalFrame) }
            } else {
                externalDragHeld = true
                heldFinderWindow = finderWindow
                heldFinderWindowFrame = originalFrame
            }
        } catch {
            if mouseIsDown { try? postMouse(type: .leftMouseUp, point: destination) }
            if let originalFrame { _ = setAXFrame(finderWindow, frame: originalFrame) }
            throw error
        }

        let current = try windowSnapshot(pid: pid, windowID: window.windowID)
        guard current.frame.equalTo(window.frame) else {
            if externalDragHeld {
                try? postMouse(type: .leftMouseUp, point: destination)
                externalDragHeld = false
                restoreHeldFinderWindow()
            }
            throw AcceptanceFailure(
                code: "PRECONDITION_WINDOW_OWNER",
                message: "Viewer window changed during Finder drag"
            )
        }
        return [
            "performed": true,
            "command": "finderDrag",
            "held": !release,
        ]
    }

    private func waitForFinder(timeoutMilliseconds: Int) throws -> NSRunningApplication {
        let deadline = Date().addingTimeInterval(Double(timeoutMilliseconds) / 1000)
        while Date() < deadline {
            if let finder = NSRunningApplication
                .runningApplications(withBundleIdentifier: "com.apple.finder")
                .first,
               finder.isActive
            {
                return finder
            }
            usleep(20_000)
        }
        throw AcceptanceFailure(
            code: "STATE_TARGET_NOT_FOUND",
            message: "Finder did not expose the selected fixture"
        )
    }

    private func focusedFinderWindow(pid: pid_t) throws -> AXUIElement {
        let application = AXUIElementCreateApplication(pid)
        if let focused = axAttribute(
            application,
            name: kAXFocusedWindowAttribute as CFString
        ), CFGetTypeID(focused) == AXUIElementGetTypeID() {
            return unsafeBitCast(focused, to: AXUIElement.self)
        }
        let windows = axAttribute(application, name: kAXWindowsAttribute as CFString)
            as? [AXUIElement] ?? []
        guard let window = windows.first else {
            throw AcceptanceFailure(
                code: "STATE_TARGET_NOT_FOUND",
                message: "Finder has no accessible source window"
            )
        }
        return window
    }

    private func stagedFinderFrame(around viewerFrame: CGRect) -> CGRect? {
        let display = CGDisplayBounds(CGMainDisplayID())
        let top = max(display.minY + 24, viewerFrame.minY)
        let height = min(620, max(360, display.maxY - top - 24))
        let rightSpace = display.maxX - viewerFrame.maxX - 16
        if rightSpace >= 300 {
            return CGRect(
                x: viewerFrame.maxX + 16,
                y: top,
                width: min(520, rightSpace),
                height: height
            )
        }
        let leftSpace = viewerFrame.minX - display.minX - 16
        if leftSpace >= 300 {
            let width = min(520, leftSpace)
            return CGRect(
                x: viewerFrame.minX - width - 16,
                y: top,
                width: width,
                height: height
            )
        }
        let width = min(360, display.width)
        return CGRect(
            x: max(display.minX, viewerFrame.maxX - width),
            y: top,
            width: width,
            height: height
        )
    }

    private func setAXFrame(_ element: AXUIElement, frame: CGRect) -> Bool {
        var point = frame.origin
        var size = frame.size
        guard let pointValue = AXValueCreate(.cgPoint, &point),
              let sizeValue = AXValueCreate(.cgSize, &size)
        else { return false }
        let positionResult = AXUIElementSetAttributeValue(
            element,
            kAXPositionAttribute as CFString,
            pointValue
        )
        let sizeResult = AXUIElementSetAttributeValue(
            element,
            kAXSizeAttribute as CFString,
            sizeValue
        )
        return positionResult == .success && sizeResult == .success
    }

    private func waitForFinderSelection(
        pid: pid_t,
        name: String,
        windowFrame: CGRect,
        timeoutMilliseconds: Int
    ) throws -> AXUIElement {
        let application = AXUIElementCreateApplication(pid)
        let deadline = Date().addingTimeInterval(Double(timeoutMilliseconds) / 1000)
        while Date() < deadline {
            var candidates: [(element: AXUIElement, score: Int, area: CGFloat)] = []
            var visited = 0

            func visit(_ element: AXUIElement, depth: Int) {
                guard depth <= 32, visited < 20_000 else { return }
                visited += 1
                if let frame = axFrame(element),
                   frame.width >= 4,
                   frame.height >= 4,
                   frame.intersects(windowFrame)
                {
                    let title = axAttribute(element, name: kAXTitleAttribute as CFString) as? String
                    let description = axAttribute(
                        element,
                        name: kAXDescriptionAttribute as CFString
                    ) as? String
                    let value = axAttribute(element, name: kAXValueAttribute as CFString) as? String
                    let elementName = [title, description, value]
                        .compactMap { $0 }
                        .first { !$0.isEmpty }
                    let selected = axAttribute(
                        element,
                        name: kAXSelectedAttribute as CFString
                    ) as? Bool ?? false
                    let role = axAttribute(element, name: kAXRoleAttribute as CFString) as? String
                    var score = selected ? 100 : 0
                    if elementName == name { score += 80 }
                    if ["AXRow", "AXCell", "AXImage", "AXGroup"].contains(role) { score += 10 }
                    if score >= 100 || elementName == name {
                        candidates.append((element, score, frame.width * frame.height))
                    }
                }
                for child in axChildren(element) { visit(child, depth: depth + 1) }
            }

            visit(application, depth: 0)
            if let match = candidates.max(by: { left, right in
                left.score == right.score ? left.area > right.area : left.score < right.score
            }) {
                return match.element
            }
            usleep(20_000)
        }
        throw AcceptanceFailure(
            code: "STATE_TARGET_NOT_FOUND",
            message: "Finder selected item was not found"
        )
    }

    private func postMouse(type: CGEventType, point: CGPoint) throws {
        guard let event = CGEvent(
            mouseEventSource: nil,
            mouseType: type,
            mouseCursorPosition: point,
            mouseButton: .left
        ) else {
            throw AcceptanceFailure(
                code: "STATE_ACTION_FAILED",
                message: "Unable to create Finder drag event"
            )
        }
        event.post(tap: .cghidEventTap)
    }

    private func restoreHeldFinderWindow() {
        if let window = heldFinderWindow, let frame = heldFinderWindowFrame {
            _ = setAXFrame(window, frame: frame)
        }
        heldFinderWindow = nil
        heldFinderWindowFrame = nil
    }

    private func focusTarget(
        payload: [String: Any],
        pid: Int,
        window: MacWindowSnapshot,
        timeoutMilliseconds: Int
    ) throws {
        let target = payload["target"] as? [String: Any] ?? [:]
        let match = try uniqueAXMatch(pid: pid, window: window, target: target)
        let ownedWindow = try axWindow(pid: pid, window: window)
        let raiseResult = AXUIElementPerformAction(ownedWindow, kAXRaiseAction as CFString)
        let mainResult = AXUIElementSetAttributeValue(
            ownedWindow,
            kAXMainAttribute as CFString,
            kCFBooleanTrue
        )
        guard let application = NSRunningApplication(processIdentifier: pid_t(pid)),
              application.activate(options: [.activateAllWindows]),
              raiseResult == .success || mainResult == .success
        else {
            throw AcceptanceFailure(
                code: "STATE_ACTION_FAILED",
                message: "Unable to activate Viewer"
            )
        }
        let titleBarPoint = CGPoint(
            x: window.frame.midX,
            y: window.frame.minY + min(14, window.frame.height / 4)
        )
        guard let titleDown = CGEvent(
            mouseEventSource: nil,
            mouseType: .leftMouseDown,
            mouseCursorPosition: titleBarPoint,
            mouseButton: .left
        ), let titleUp = CGEvent(
            mouseEventSource: nil,
            mouseType: .leftMouseUp,
            mouseCursorPosition: titleBarPoint,
            mouseButton: .left
        ) else {
            throw AcceptanceFailure(
                code: "STATE_ACTION_FAILED",
                message: "Unable to create Viewer activation event"
            )
        }
        titleDown.post(tap: .cghidEventTap)
        titleUp.post(tap: .cghidEventTap)
        try bringProcessFrontmost(pid)
        guard AXUIElementSetAttributeValue(
            match.element,
            kAXFocusedAttribute as CFString,
            kCFBooleanTrue
        ) == .success else {
            throw AcceptanceFailure(
                code: "STATE_ACTION_FAILED",
                message: "Accessibility focus action failed"
            )
        }

        let deadline = Date().addingTimeInterval(Double(timeoutMilliseconds) / 1000)
        while Date() < deadline {
            if try windowSnapshot(pid: pid, windowID: window.windowID).frontmost { return }
            usleep(20_000)
        }
        throw AcceptanceFailure(
            code: "PRECONDITION_WINDOW_OWNER",
            message: "Viewer did not become frontmost"
        )
    }

    private func bringProcessFrontmost(_ pid: Int) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
        process.arguments = [
            "-e",
            "tell application \"System Events\" to set frontmost of first process whose unix id is \(pid) to true",
        ]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            throw AcceptanceFailure(
                code: "STATE_ACTION_FAILED",
                message: "Unable to request Viewer activation"
            )
        }
        guard process.terminationStatus == 0 else {
            throw AcceptanceFailure(
                code: "STATE_ACTION_FAILED",
                message: "System Events rejected Viewer activation"
            )
        }
    }

    func capture(
        path: String,
        pid: Int,
        window: MacWindowSnapshot
    ) throws -> [String: Any] {
        try requireStableWindow(
            pid: pid,
            window: window,
            requireFrontmost: !externalDragHeld
        )
        let image = try captureImage(pid: pid, window: window)

        let scaleX = Double(image.width) / window.frame.width
        let scaleY = Double(image.height) / window.frame.height
        guard scaleX >= 1,
              scaleX <= 3,
              abs(scaleX - scaleY) < 0.02
        else {
            throw AcceptanceFailure(
                code: "CAPTURE_WINDOW_MISMATCH",
                message: "Captured image dimensions do not match the Viewer window"
            )
        }

        let url = URL(fileURLWithPath: path)
        guard let destination = CGImageDestinationCreateWithURL(
            url as CFURL,
            UTType.png.identifier as CFString,
            1,
            nil
        ) else {
            throw AcceptanceFailure(
                code: "CAPTURE_WRITE_FAILED",
                message: "Unable to create PNG destination"
            )
        }
        CGImageDestinationAddImage(destination, image, nil)
        guard CGImageDestinationFinalize(destination) else {
            try? FileManager.default.removeItem(at: url)
            throw AcceptanceFailure(
                code: "CAPTURE_WRITE_FAILED",
                message: "Unable to finalize PNG evidence"
            )
        }
        let data: Data
        do {
            data = try Data(contentsOf: url)
        } catch {
            try? FileManager.default.removeItem(at: url)
            throw AcceptanceFailure(
                code: "CAPTURE_WRITE_FAILED",
                message: "Unable to read PNG evidence"
            )
        }
        let digest = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
        return [
            "performed": true,
            "command": "capture",
            "width": image.width,
            "height": image.height,
            "sha256": digest,
        ]
    }

    private func captureImage(pid: Int, window: MacWindowSnapshot) throws -> CGImage {
        let semaphore = DispatchSemaphore(value: 0)
        var capturedImage: CGImage?
        var captureFailure: AcceptanceFailure?

        SCShareableContent.getExcludingDesktopWindows(
            true,
            onScreenWindowsOnly: true
        ) { content, error in
            guard error == nil, let content else {
                captureFailure = AcceptanceFailure(
                    code: "CAPTURE_WINDOW_MISMATCH",
                    message: "Unable to enumerate shareable windows"
                )
                semaphore.signal()
                return
            }
            let matches = content.windows.filter { candidate in
                Int(candidate.windowID) == window.windowID &&
                    candidate.owningApplication?.processID == pid_t(pid)
            }
            guard matches.count == 1 else {
                captureFailure = AcceptanceFailure(
                    code: "CAPTURE_WINDOW_MISMATCH",
                    message: "Target window changed"
                )
                semaphore.signal()
                return
            }

            let filter = SCContentFilter(desktopIndependentWindow: matches[0])
            let configuration = SCStreamConfiguration()
            let scale = max(1, min(3, CGFloat(filter.pointPixelScale)))
            configuration.width = Int(window.frame.width * scale)
            configuration.height = Int(window.frame.height * scale)
            configuration.showsCursor = false
            configuration.ignoreShadowsSingleWindow = true
            SCScreenshotManager.captureImage(
                contentFilter: filter,
                configuration: configuration
            ) { image, error in
                if let image, error == nil {
                    capturedImage = image
                } else {
                    captureFailure = AcceptanceFailure(
                        code: "CAPTURE_WINDOW_MISMATCH",
                        message: "Unable to capture the approved Viewer window"
                    )
                }
                semaphore.signal()
            }
        }

        guard semaphore.wait(timeout: .now() + 10) == .success else {
            throw AcceptanceFailure(
                code: "CAPTURE_WINDOW_MISMATCH",
                message: "Window capture timed out"
            )
        }
        if let captureFailure { throw captureFailure }
        guard let capturedImage else {
            throw AcceptanceFailure(
                code: "CAPTURE_WINDOW_MISMATCH",
                message: "Window capture returned no image"
            )
        }
        return capturedImage
    }

    private func requireStableFrontmostWindow(
        pid: Int,
        window: MacWindowSnapshot
    ) throws {
        try requireStableWindow(pid: pid, window: window, requireFrontmost: true)
    }

    private func requireStableWindow(
        pid: Int,
        window: MacWindowSnapshot,
        requireFrontmost: Bool
    ) throws {
        let current = try windowSnapshot(pid: pid, windowID: window.windowID)
        guard current.frame.equalTo(window.frame), !requireFrontmost || current.frontmost else {
            throw AcceptanceFailure(
                code: "PRECONDITION_WINDOW_OWNER",
                message: "Target window changed"
            )
        }
    }

    private func uniqueAXMatch(
        pid: Int,
        window: MacWindowSnapshot,
        target: [String: Any]
    ) throws -> AXMatch {
        let root = try axWindow(pid: pid, window: window)
        var matches: [AXMatch] = []
        var visited = 0

        func visit(_ element: AXUIElement, path: [Int], depth: Int) throws {
            guard depth <= 32, visited < 20_000 else {
                throw AcceptanceFailure(
                    code: "STATE_ACTION_FAILED",
                    message: "Accessibility tree exceeds the safe traversal limit"
                )
            }
            visited += 1
            let snapshot = axSnapshot(element: element, path: path)
            if matchesTarget(snapshot, target: target) {
                matches.append(AXMatch(element: element, dictionary: snapshot))
            }
            for (index, child) in axChildren(element).enumerated() {
                try visit(child, path: path + [index], depth: depth + 1)
            }
        }

        try visit(root, path: [], depth: 0)
        guard !matches.isEmpty else {
            throw AcceptanceFailure(
                code: "STATE_TARGET_NOT_FOUND",
                message: "Target element was not found"
            )
        }
        if target["position"] as? String == "rightmost" {
            let visibleMatches = matches.filter { match in
                guard
                    let frame = match.dictionary["frame"] as? [String: Int],
                    let x = frame["x"],
                    let y = frame["y"],
                    let width = frame["width"],
                    let height = frame["height"]
                else { return false }
                return x + width > Int(window.frame.minX) &&
                    x < Int(window.frame.maxX) &&
                    y + height > Int(window.frame.minY) &&
                    y < Int(window.frame.maxY)
            }
            if let rightmost = visibleMatches.max(by: { left, right in
                let leftFrame = left.dictionary["frame"] as? [String: Int] ?? [:]
                let rightFrame = right.dictionary["frame"] as? [String: Int] ?? [:]
                return (leftFrame["x"] ?? 0) < (rightFrame["x"] ?? 0)
            }) {
                matches = [rightmost]
            }
        }
        guard matches.count == 1 else {
            throw AcceptanceFailure(
                code: "STATE_TARGET_NOT_UNIQUE",
                message: "Target element was not unique"
            )
        }
        return matches[0]
    }

    private func axWindow(pid: Int, window: MacWindowSnapshot) throws -> AXUIElement {
        let application = AXUIElementCreateApplication(pid_t(pid))
        let windows = axAttribute(application, name: kAXWindowsAttribute as CFString)
            as? [AXUIElement] ?? []
        let matches = windows.filter { candidate in
            if let number = axAttribute(candidate, name: "AXWindowNumber" as CFString) as? Int {
                return number == window.windowID
            }
            guard let frame = axFrame(candidate) else { return false }
            return abs(frame.origin.x - window.frame.origin.x) < 1 &&
                abs(frame.origin.y - window.frame.origin.y) < 1 &&
                abs(frame.width - window.frame.width) < 1 &&
                abs(frame.height - window.frame.height) < 1
        }
        guard matches.count == 1 else {
            throw AcceptanceFailure(
                code: "PRECONDITION_WINDOW_OWNER",
                message: "Target window changed"
            )
        }
        return matches[0]
    }

    private func axAttribute(_ element: AXUIElement, name: CFString) -> AnyObject? {
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, name, &value) == .success else {
            return nil
        }
        return value
    }

    private func axChildren(_ element: AXUIElement) -> [AXUIElement] {
        axAttribute(element, name: kAXChildrenAttribute as CFString) as? [AXUIElement] ?? []
    }

    private func axFrame(_ element: AXUIElement) -> CGRect? {
        guard
            let rawPosition = axAttribute(element, name: kAXPositionAttribute as CFString),
            let rawSize = axAttribute(element, name: kAXSizeAttribute as CFString),
            CFGetTypeID(rawPosition) == AXValueGetTypeID(),
            CFGetTypeID(rawSize) == AXValueGetTypeID()
        else { return nil }

        var point = CGPoint.zero
        var size = CGSize.zero
        let position = unsafeBitCast(rawPosition, to: AXValue.self)
        let dimensions = unsafeBitCast(rawSize, to: AXValue.self)
        guard AXValueGetValue(position, .cgPoint, &point),
              AXValueGetValue(dimensions, .cgSize, &size)
        else { return nil }
        return CGRect(origin: point, size: size)
    }

    private func axSnapshot(element: AXUIElement, path: [Int]) -> [String: Any] {
        let role = axAttribute(element, name: kAXRoleAttribute as CFString) as? String ?? ""
        let title = axAttribute(element, name: kAXTitleAttribute as CFString) as? String
        let description = axAttribute(element, name: kAXDescriptionAttribute as CFString) as? String
        let value = axAttribute(element, name: kAXValueAttribute as CFString) as? String
        let identifier = axAttribute(element, name: kAXIdentifierAttribute as CFString) as? String ?? ""
        let enabled = axAttribute(element, name: kAXEnabledAttribute as CFString) as? Bool ?? false
        let focused = axAttribute(element, name: kAXFocusedAttribute as CFString) as? Bool ?? false
        let frame = axFrame(element) ?? .zero
        let name = [title, description, value]
            .compactMap { $0 }
            .first { !$0.isEmpty } ?? ""
        return [
            "role": role,
            "name": name,
            "identifier": identifier,
            "enabled": enabled,
            "focused": focused,
            "frame": [
                "x": Int(frame.origin.x.rounded()),
                "y": Int(frame.origin.y.rounded()),
                "width": Int(frame.width.rounded()),
                "height": Int(frame.height.rounded()),
            ],
            "path": path,
        ]
    }

    private func matchesTarget(_ snapshot: [String: Any], target: [String: Any]) -> Bool {
        for key in ["role", "name", "identifier"] {
            if let expected = target[key] as? String,
               snapshot[key] as? String != expected
            {
                return false
            }
        }
        if let expectedPrefix = target["namePrefix"] as? String {
            guard let name = snapshot["name"] as? String,
                  name.hasPrefix(expectedPrefix)
            else { return false }
        }
        return true
    }

    private func postKey(_ payload: [String: Any]) throws {
        let keyCodes: [String: CGKeyCode] = [
            "tab": 48, "enter": 36, "space": 49, "escape": 53,
            "arrowUp": 126, "arrowDown": 125, "arrowLeft": 123, "arrowRight": 124,
            "home": 115, "period": 47, "end": 119, "delete": 117, "backspace": 51,
            "f10": 109,
            "a": 0, "b": 11, "c": 8, "d": 2, "e": 14, "f": 3, "g": 5,
            "h": 4, "i": 34, "j": 38, "k": 40, "l": 37, "m": 46, "n": 45,
            "o": 31, "p": 35, "q": 12, "r": 15, "s": 1, "t": 17, "u": 32,
            "v": 9, "w": 13, "x": 7, "y": 16, "z": 6,
            "0": 29, "1": 18, "2": 19, "3": 20, "4": 21,
            "5": 23, "6": 22, "7": 26, "8": 28, "9": 25,
        ]
        guard let key = payload["key"] as? String,
              let keyCode = keyCodes[key],
              let modifiers = payload["modifiers"] as? [String]
        else {
            throw AcceptanceFailure(code: "SAFETY_COMMAND", message: "Invalid key payload")
        }
        var flags: CGEventFlags = []
        for modifier in modifiers {
            switch modifier {
            case "shift": flags.insert(.maskShift)
            case "control": flags.insert(.maskControl)
            case "option": flags.insert(.maskAlternate)
            case "command": flags.insert(.maskCommand)
            default: break
            }
        }
        guard let down = CGEvent(keyboardEventSource: nil, virtualKey: keyCode, keyDown: true),
              let up = CGEvent(keyboardEventSource: nil, virtualKey: keyCode, keyDown: false)
        else {
            throw AcceptanceFailure(code: "STATE_ACTION_FAILED", message: "Unable to create key event")
        }
        down.flags = flags
        up.flags = flags
        let unicodeText: String? = if key == "space" {
            " "
        } else if key == "period" {
            "."
        } else if key.count == 1 {
            key
        } else {
            nil
        }
        if let unicodeText {
            let unicodeUnits = Array(unicodeText.utf16)
            unicodeUnits.withUnsafeBufferPointer { buffer in
                guard let baseAddress = buffer.baseAddress else { return }
                down.keyboardSetUnicodeString(
                    stringLength: buffer.count,
                    unicodeString: baseAddress
                )
                up.keyboardSetUnicodeString(
                    stringLength: buffer.count,
                    unicodeString: baseAddress
                )
            }
        }
        down.post(tap: .cghidEventTap)
        up.post(tap: .cghidEventTap)
    }

    private func screenPoint(_ value: Any?, window: MacWindowSnapshot) throws -> CGPoint {
        guard let point = value as? [String: Any],
              let x = numberValue(point["x"]),
              let y = numberValue(point["y"]),
              x >= 0,
              y >= 0,
              x < window.frame.width,
              y < window.frame.height
        else {
            throw AcceptanceFailure(
                code: "SAFETY_POINT_OUTSIDE_WINDOW",
                message: "Pointer coordinate is outside the approved window"
            )
        }
        return CGPoint(x: window.frame.origin.x + x, y: window.frame.origin.y + y)
    }

    private func postPointer(
        _ payload: [String: Any],
        pid: Int,
        window: MacWindowSnapshot
    ) throws {
        guard let kind = payload["kind"] as? String else {
            throw AcceptanceFailure(code: "SAFETY_COMMAND", message: "Invalid pointer payload")
        }
        let point = try screenPoint(payload["point"], window: window)
        if kind == "move" {
            guard let event = CGEvent(
                mouseEventSource: nil,
                mouseType: .mouseMoved,
                mouseCursorPosition: point,
                mouseButton: .left
            ) else {
                throw AcceptanceFailure(
                    code: "STATE_ACTION_FAILED",
                    message: "Unable to create pointer move event"
                )
            }
            event.post(tap: .cghidEventTap)
            return
        }
        if kind == "scroll" {
            CGEvent(
                mouseEventSource: nil,
                mouseType: .mouseMoved,
                mouseCursorPosition: point,
                mouseButton: .left
            )?.post(tap: .cghidEventTap)
            guard
                let deltaY = payload["deltaY"] as? Int,
                let event = CGEvent(
                    scrollWheelEvent2Source: nil,
                    units: .line,
                    wheelCount: 1,
                    wheel1: Int32(deltaY),
                    wheel2: 0,
                    wheel3: 0
                )
            else {
                throw AcceptanceFailure(
                    code: "STATE_ACTION_FAILED",
                    message: "Unable to create scroll event"
                )
            }
            event.post(tap: .cghidEventTap)
            return
        }
        if ["leftDown", "leftDrag", "leftUp", "rightDown", "rightDrag", "rightUp"].contains(kind) {
            let eventType: CGEventType = switch kind {
            case "leftDown": .leftMouseDown
            case "leftDrag": .leftMouseDragged
            case "leftUp": .leftMouseUp
            case "rightDown": .rightMouseDown
            case "rightDrag": .rightMouseDragged
            default: .rightMouseUp
            }
            var flags: CGEventFlags = []
            for modifier in payload["modifiers"] as? [String] ?? [] {
                switch modifier {
                case "shift": flags.insert(.maskShift)
                case "control": flags.insert(.maskControl)
                case "option": flags.insert(.maskAlternate)
                case "command": flags.insert(.maskCommand)
                default: break
                }
            }
            let mouseButton: CGMouseButton = kind.hasPrefix("right") ? .right : .left
            guard let event = CGEvent(
                mouseEventSource: nil,
                mouseType: eventType,
                mouseCursorPosition: point,
                mouseButton: mouseButton
            ) else {
                throw AcceptanceFailure(
                    code: "STATE_ACTION_FAILED",
                    message: "Unable to create held pointer event"
                )
            }
            event.flags = flags
            event.post(tap: .cghidEventTap)
            return
        }
        let button: CGMouseButton = kind == "rightClick" ? .right : .left
        let downType: CGEventType = kind == "rightClick" ? .rightMouseDown : .leftMouseDown
        let upType: CGEventType = kind == "rightClick" ? .rightMouseUp : .leftMouseUp
        let clickCount = kind == "doubleClick" ? 2 : 1
        var flags: CGEventFlags = []
        for modifier in payload["modifiers"] as? [String] ?? [] {
            switch modifier {
            case "shift": flags.insert(.maskShift)
            case "control": flags.insert(.maskControl)
            case "option": flags.insert(.maskAlternate)
            case "command": flags.insert(.maskCommand)
            default: break
            }
        }
        let eventSource = CGEventSource(stateID: .hidSystemState)
        for clickIndex in 1 ... clickCount {
            guard let down = CGEvent(
                mouseEventSource: eventSource,
                mouseType: downType,
                mouseCursorPosition: point,
                mouseButton: button
            ), let up = CGEvent(
                mouseEventSource: eventSource,
                mouseType: upType,
                mouseCursorPosition: point,
                mouseButton: button
            ) else {
                throw AcceptanceFailure(
                    code: "STATE_ACTION_FAILED",
                    message: "Unable to create pointer event"
                )
            }
            down.setIntegerValueField(.mouseEventClickState, value: Int64(clickIndex))
            up.setIntegerValueField(.mouseEventClickState, value: Int64(clickIndex))
            let eventNumber = Int64(
                (DispatchTime.now().uptimeNanoseconds / 1_000_000) & UInt64(Int32.max)
            )
            down.setIntegerValueField(.mouseEventNumber, value: eventNumber)
            up.setIntegerValueField(.mouseEventNumber, value: eventNumber)
            down.flags = flags
            up.flags = flags
            down.timestamp = CGEventTimestamp(DispatchTime.now().uptimeNanoseconds)
            if kind == "doubleClick" {
                down.postToPid(pid_t(pid))
            } else {
                down.post(tap: .cghidEventTap)
            }
            usleep(12_000)
            up.timestamp = CGEventTimestamp(DispatchTime.now().uptimeNanoseconds)
            if kind == "doubleClick" {
                up.postToPid(pid_t(pid))
            } else {
                up.post(tap: .cghidEventTap)
            }
            if clickIndex < clickCount {
                usleep(150_000)
            }
        }
    }

    private func postDrag(_ payload: [String: Any], window: MacWindowSnapshot) throws {
        let start = try screenPoint(payload["from"], window: window)
        let end = try screenPoint(payload["to"], window: window)
        guard let duration = payload["durationMs"] as? Int,
              let down = CGEvent(
                  mouseEventSource: nil,
                  mouseType: .leftMouseDown,
                  mouseCursorPosition: start,
                  mouseButton: .left
              )
        else {
            throw AcceptanceFailure(code: "STATE_ACTION_FAILED", message: "Unable to start drag")
        }
        down.post(tap: .cghidEventTap)
        defer {
            CGEvent(
                mouseEventSource: nil,
                mouseType: .leftMouseUp,
                mouseCursorPosition: end,
                mouseButton: .left
            )?.post(tap: .cghidEventTap)
        }
        let steps = max(2, min(120, duration / 10))
        for step in 1 ... steps {
            let progress = Double(step) / Double(steps)
            let point = CGPoint(
                x: start.x + (end.x - start.x) * progress,
                y: start.y + (end.y - start.y) * progress
            )
            CGEvent(
                mouseEventSource: nil,
                mouseType: .leftMouseDragged,
                mouseCursorPosition: point,
                mouseButton: .left
            )?.post(tap: .cghidEventTap)
            usleep(useconds_t((duration * 1000) / steps))
        }
    }
}

private func parseRequest(_ line: String) throws -> RequestEnvelope {
    guard
        let data = line.data(using: .utf8),
        let object = try? JSONSerialization.jsonObject(with: data),
        let request = object as? [String: Any]
    else {
        throw AcceptanceFailure(
            code: "SAFETY_PROTOCOL",
            message: "Malformed JSON request"
        )
    }

    let sequence = request["sequence"] as? Int ?? 0
    guard let command = request["command"] as? String, allowedCommands.contains(command) else {
        throw AcceptanceFailure(
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
        throw AcceptanceFailure(
            code: "SAFETY_PROTOCOL",
            message: "Invalid protocol envelope"
        )
    }

    return RequestEnvelope(
        sequence: sequence,
        pid: pid,
        windowID: windowID,
        command: command,
        timeoutMilliseconds: timeout,
        payload: request["payload"] as? [String: Any] ?? [:]
    )
}

private func handleLine(_ line: String, adapter: any NativeAdapter) -> [String: Any] {
    let sequence: Int
    if
        let data = line.data(using: .utf8),
        let object = try? JSONSerialization.jsonObject(with: data),
        let request = object as? [String: Any]
    {
        sequence = request["sequence"] as? Int ?? 0
    } else {
        sequence = 0
    }

    do {
        let request = try parseRequest(line)
        try validateRequestPayload(request)
        let result = try adapter.handle(request)
        return [
            "version": protocolVersion,
            "sequence": request.sequence,
            "ok": true,
            "result": result,
        ]
    } catch let failure as AcceptanceFailure {
        return errorResponse(
            sequence: sequence,
            code: failure.code,
            message: failure.message
        )
    } catch {
        return errorResponse(
            sequence: sequence,
            code: "STATE_ACTION_FAILED",
            message: "Native action failed"
        )
    }
}

private func discoverWindows(pid: Int, protocolTest: Bool) throws -> [[String: Any]] {
    if protocolTest {
        return [[
            "pid": pid,
            "windowId": 44,
            "title": "Viewer Discovery Fixture",
            "x": 20,
            "y": 30,
            "width": 1024,
            "height": 720,
        ]]
    }
    errno = 0
    if kill(pid_t(pid), 0) == -1, errno == ESRCH {
        throw AcceptanceFailure(
            code: "PRECONDITION_VIEWER_PID",
            message: "Viewer PID is not running"
        )
    }
    guard
        let rawWindows = CGWindowListCopyWindowInfo(
            [.optionOnScreenOnly, .excludeDesktopElements],
            kCGNullWindowID
        ) as? [[String: Any]]
    else {
        throw AcceptanceFailure(
            code: "PRECONDITION_WINDOW_COUNT",
            message: "Unable to discover Viewer windows"
        )
    }
    return rawWindows.compactMap { item in
        guard
            item[kCGWindowOwnerPID as String] as? Int == pid,
            item[kCGWindowLayer as String] as? Int == 0,
            let windowID = item[kCGWindowNumber as String] as? Int,
            let bounds = item[kCGWindowBounds as String] as? [String: Any],
            let frame = CGRect(dictionaryRepresentation: bounds as CFDictionary),
            frame.width > 0,
            frame.height > 0
        else {
            return nil
        }
        return [
            "pid": pid,
            "windowId": windowID,
            "title": item[kCGWindowName as String] as? String ?? "Viewer",
            "x": Int(frame.origin.x.rounded()),
            "y": Int(frame.origin.y.rounded()),
            "width": Int(frame.width.rounded()),
            "height": Int(frame.height.rounded()),
        ]
    }
}

if let discoveryIndex = CommandLine.arguments.firstIndex(of: "--discover-windows") {
    do {
        let pidIndex = CommandLine.arguments.index(after: discoveryIndex)
        guard
            pidIndex < CommandLine.arguments.endIndex,
            let pid = Int(CommandLine.arguments[pidIndex]),
            pid > 0
        else {
            throw AcceptanceFailure(
                code: "SAFETY_PROTOCOL",
                message: "Window discovery requires a positive PID"
            )
        }
        let windows = try discoverWindows(
            pid: pid,
            protocolTest: CommandLine.arguments.contains("--protocol-test-window-discovery")
        )
        writeResponse(["windows": windows])
        exit(EXIT_SUCCESS)
    } catch let failure as AcceptanceFailure {
        writeResponse(["error": ["code": failure.code, "message": failure.message]])
        exit(EXIT_FAILURE)
    } catch {
        writeResponse([
            "error": [
                "code": "PRECONDITION_WINDOW_COUNT",
                "message": "Window discovery failed",
            ],
        ])
        exit(EXIT_FAILURE)
    }
}

private let adapter: any NativeAdapter = if CommandLine.arguments.contains("--protocol-test-fixture") {
    FixtureAdapter()
} else if CommandLine.arguments.contains("--protocol-test-mac-fixture") {
    MacAdapter(system: RecordingMacSystem())
} else if CommandLine.arguments.contains("--protocol-test-mac-background-fixture") {
    MacAdapter(system: RecordingMacSystem(frontmost: false))
} else if CommandLine.arguments.contains("--protocol-test") {
    ProtocolTestAdapter()
} else {
    MacAdapter(system: LiveMacSystem())
}

while let line = readLine() {
    writeResponse(handleLine(line, adapter: adapter))
}
