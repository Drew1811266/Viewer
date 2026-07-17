import CoreGraphics
import CryptoKit
import Foundation
import ImageIO
import UniformTypeIdentifiers

struct FixtureMetadata: Encodable {
    let format: String
    let width: Int
    let height: Int
    let orientation: Int
    let hasAlpha: Bool
    let profile: String?
    let sha256: String
}

struct FixtureManifest: Encodable {
    let fixtures: [String: FixtureMetadata]
}

enum FixtureError: Error {
    case contextCreation
    case imageCreation
    case destinationCreation
    case destinationFinalize
}

let repositoryRoot = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
let outputDirectory = repositoryRoot.appending(path: "tests/fixtures/images")
try FileManager.default.createDirectory(
    at: outputDirectory,
    withIntermediateDirectories: true
)

func makeImage(
    width: Int,
    height: Int,
    colorSpace: CGColorSpace,
    transparent: Bool
) throws -> CGImage {
    let alpha = transparent ? CGImageAlphaInfo.premultipliedLast : .noneSkipLast
    let bitmapInfo = CGBitmapInfo.byteOrder32Big.rawValue | alpha.rawValue
    guard let context = CGContext(
        data: nil,
        width: width,
        height: height,
        bitsPerComponent: 8,
        bytesPerRow: width * 4,
        space: colorSpace,
        bitmapInfo: bitmapInfo
    ) else {
        throw FixtureError.contextCreation
    }

    if transparent {
        context.clear(CGRect(x: 0, y: 0, width: width, height: height))
        context.setFillColor([0.2, 0.5, 0.9, 0.55])
        context.fillEllipse(in: CGRect(x: 64, y: 48, width: width - 128, height: height - 96))
    } else {
        context.setFillColor([0.12, 0.32, 0.82, 1.0])
        context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        context.setFillColor([0.96, 0.35, 0.12, 1.0])
        context.fill(CGRect(x: width / 4, y: height / 4, width: width / 2, height: height / 2))
    }

    guard let image = context.makeImage() else {
        throw FixtureError.imageCreation
    }
    return image
}

func write(
    image: CGImage,
    name: String,
    type: UTType,
    orientation: Int = 1
) throws {
    let url = outputDirectory.appending(path: name)
    guard let destination = CGImageDestinationCreateWithURL(
        url as CFURL,
        type.identifier as CFString,
        1,
        nil
    ) else {
        throw FixtureError.destinationCreation
    }

    let properties: [CFString: Any] = [
        kCGImagePropertyOrientation: orientation,
        kCGImageDestinationLossyCompressionQuality: 0.9,
    ]
    CGImageDestinationAddImage(destination, image, properties as CFDictionary)
    guard CGImageDestinationFinalize(destination) else {
        throw FixtureError.destinationFinalize
    }
}

let sRGB = CGColorSpace(name: CGColorSpace.sRGB)!
let displayP3 = CGColorSpace(name: CGColorSpace.displayP3)!

try write(
    image: makeImage(width: 1024, height: 768, colorSpace: sRGB, transparent: false),
    name: "srgb.jpg",
    type: .jpeg
)
try write(
    image: makeImage(width: 1024, height: 768, colorSpace: displayP3, transparent: false),
    name: "p3.jpg",
    type: .jpeg
)
try write(
    image: makeImage(width: 800, height: 600, colorSpace: sRGB, transparent: false),
    name: "rotated-6.jpg",
    type: .jpeg,
    orientation: 6
)
try write(
    image: makeImage(width: 640, height: 480, colorSpace: sRGB, transparent: true),
    name: "alpha.png",
    type: .png
)

let jpegData = try Data(contentsOf: outputDirectory.appending(path: "srgb.jpg"))
try jpegData.prefix(64).write(to: outputDirectory.appending(path: "corrupt.jpg"))

func checksum(_ name: String) throws -> String {
    let data = try Data(contentsOf: outputDirectory.appending(path: name))
    return SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
}

let manifest = FixtureManifest(fixtures: [
    "alpha.png": FixtureMetadata(
        format: "png",
        width: 640,
        height: 480,
        orientation: 1,
        hasAlpha: true,
        profile: "sRGB IEC61966-2.1",
        sha256: try checksum("alpha.png")
    ),
    "corrupt.jpg": FixtureMetadata(
        format: "jpeg",
        width: 0,
        height: 0,
        orientation: 1,
        hasAlpha: false,
        profile: nil,
        sha256: try checksum("corrupt.jpg")
    ),
    "p3.jpg": FixtureMetadata(
        format: "jpeg",
        width: 1024,
        height: 768,
        orientation: 1,
        hasAlpha: false,
        profile: "Display P3",
        sha256: try checksum("p3.jpg")
    ),
    "rotated-6.jpg": FixtureMetadata(
        format: "jpeg",
        width: 800,
        height: 600,
        orientation: 6,
        hasAlpha: false,
        profile: "sRGB IEC61966-2.1",
        sha256: try checksum("rotated-6.jpg")
    ),
    "srgb.jpg": FixtureMetadata(
        format: "jpeg",
        width: 1024,
        height: 768,
        orientation: 1,
        hasAlpha: false,
        profile: "sRGB IEC61966-2.1",
        sha256: try checksum("srgb.jpg")
    ),
])

let encoder = JSONEncoder()
encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
var manifestData = try encoder.encode(manifest)
manifestData.append(0x0A)
try manifestData.write(to: outputDirectory.appending(path: "manifest.json"))
