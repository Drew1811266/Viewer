import CoreGraphics
import Foundation
import ImageIO

struct PairReport: Encodable {
    let fixture: String
    let width: Int
    let height: Int
    let colorSpace: String
    let meanAbsoluteDelta: Double
    let maximumChannelDelta: Int
    let significantPixelRatio: Double
    let passed: Bool
}

struct ConsistencyReport: Encodable {
    let schemaVersion: Int
    let meanAbsoluteDeltaLimit: Double
    let significantPixelDelta: Int
    let significantPixelRatioLimit: Double
    let pairs: [PairReport]
    let passed: Bool
}

enum ConsistencyError: Error, CustomStringConvertible {
    case usage
    case noPairs
    case imageLoad(String)
    case dimensionMismatch(String)
    case contextCreation(String)

    var description: String {
        switch self {
        case .usage:
            return "usage: swift check-g1-image-consistency.swift <artifact-directory> <report-path>"
        case .noPairs:
            return "no Quick Look/Image I/O artifact pairs were found"
        case .imageLoad(let path):
            return "failed to load image artifact: \(path)"
        case .dimensionMismatch(let fixture):
            return "backend dimensions differ for \(fixture)"
        case .contextCreation(let fixture):
            return "failed to create comparison context for \(fixture)"
        }
    }
}

let meanAbsoluteDeltaLimit = 2.0
let significantPixelDelta = 8
let significantPixelRatioLimit = 0.02

func loadImage(_ url: URL) throws -> CGImage {
    guard
        let source = CGImageSourceCreateWithURL(url as CFURL, nil),
        let image = CGImageSourceCreateImageAtIndex(source, 0, nil)
    else {
        throw ConsistencyError.imageLoad(url.path)
    }
    return image
}

func render(
    _ image: CGImage,
    colorSpace: CGColorSpace,
    fixture: String
) throws -> [UInt8] {
    let bytesPerRow = image.width * 4
    var pixels = [UInt8](repeating: 0, count: bytesPerRow * image.height)
    let created = pixels.withUnsafeMutableBytes { bytes -> Bool in
        guard let context = CGContext(
            data: bytes.baseAddress,
            width: image.width,
            height: image.height,
            bitsPerComponent: 8,
            bytesPerRow: bytesPerRow,
            space: colorSpace,
            bitmapInfo: CGBitmapInfo.byteOrder32Big.rawValue
                | CGImageAlphaInfo.premultipliedLast.rawValue
        ) else {
            return false
        }
        context.setBlendMode(.copy)
        context.interpolationQuality = .none
        context.draw(
            image,
            in: CGRect(x: 0, y: 0, width: image.width, height: image.height)
        )
        return true
    }
    guard created else {
        throw ConsistencyError.contextCreation(fixture)
    }
    return pixels
}

func compare(
    fixture: String,
    quickLookURL: URL,
    imageIoURL: URL
) throws -> PairReport {
    let quickLook = try loadImage(quickLookURL)
    let imageIo = try loadImage(imageIoURL)
    guard quickLook.width == imageIo.width, quickLook.height == imageIo.height else {
        throw ConsistencyError.dimensionMismatch(fixture)
    }

    let usesDisplayP3 = fixture.hasPrefix("p3-")
    let colorSpaceName = usesDisplayP3 ? CGColorSpace.displayP3 : CGColorSpace.sRGB
    let colorSpaceLabel = usesDisplayP3 ? "Display P3" : "sRGB"
    guard let colorSpace = CGColorSpace(name: colorSpaceName) else {
        throw ConsistencyError.contextCreation(fixture)
    }
    let quickLookPixels = try render(quickLook, colorSpace: colorSpace, fixture: fixture)
    let imageIoPixels = try render(imageIo, colorSpace: colorSpace, fixture: fixture)

    var totalDelta: UInt64 = 0
    var maximumDelta = 0
    var significantPixels = 0
    let pixelCount = quickLook.width * quickLook.height
    for pixel in 0..<pixelCount {
        var pixelMaximum = 0
        for channel in 0..<4 {
            let index = pixel * 4 + channel
            let delta = abs(Int(quickLookPixels[index]) - Int(imageIoPixels[index]))
            totalDelta += UInt64(delta)
            maximumDelta = max(maximumDelta, delta)
            pixelMaximum = max(pixelMaximum, delta)
        }
        if pixelMaximum > significantPixelDelta {
            significantPixels += 1
        }
    }
    let componentCount = Double(pixelCount * 4)
    let meanAbsoluteDelta = Double(totalDelta) / componentCount
    let significantPixelRatio = Double(significantPixels) / Double(pixelCount)
    let passed = meanAbsoluteDelta <= meanAbsoluteDeltaLimit
        && significantPixelRatio <= significantPixelRatioLimit

    return PairReport(
        fixture: fixture,
        width: quickLook.width,
        height: quickLook.height,
        colorSpace: colorSpaceLabel,
        meanAbsoluteDelta: meanAbsoluteDelta,
        maximumChannelDelta: maximumDelta,
        significantPixelRatio: significantPixelRatio,
        passed: passed
    )
}

do {
    guard CommandLine.arguments.count == 3 else {
        throw ConsistencyError.usage
    }
    let directory = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
    let reportURL = URL(fileURLWithPath: CommandLine.arguments[2])
    let files = try FileManager.default.contentsOfDirectory(
        at: directory,
        includingPropertiesForKeys: nil
    )
    let quickLookFiles = files
        .filter { $0.lastPathComponent.hasSuffix("-quick-look.png") }
        .sorted { $0.lastPathComponent < $1.lastPathComponent }
    guard !quickLookFiles.isEmpty else {
        throw ConsistencyError.noPairs
    }

    var pairs = [PairReport]()
    for quickLookURL in quickLookFiles {
        let fixture = String(
            quickLookURL.lastPathComponent.dropLast("-quick-look.png".count)
        )
        let imageIoURL = directory.appending(path: "\(fixture)-image-io.png")
        pairs.append(
            try compare(
                fixture: fixture,
                quickLookURL: quickLookURL,
                imageIoURL: imageIoURL
            )
        )
    }
    let report = ConsistencyReport(
        schemaVersion: 1,
        meanAbsoluteDeltaLimit: meanAbsoluteDeltaLimit,
        significantPixelDelta: significantPixelDelta,
        significantPixelRatioLimit: significantPixelRatioLimit,
        pairs: pairs,
        passed: pairs.allSatisfy(\.passed)
    )
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
    var data = try encoder.encode(report)
    data.append(0x0A)
    try FileManager.default.createDirectory(
        at: reportURL.deletingLastPathComponent(),
        withIntermediateDirectories: true
    )
    try data.write(to: reportURL)
    print("G1 consistency report: \(reportURL.path)")
    print("Consistency gate passed: \(report.passed)")
    if !report.passed {
        exit(1)
    }
} catch {
    fputs("G1 consistency check failed: \(error)\n", stderr)
    exit(1)
}
