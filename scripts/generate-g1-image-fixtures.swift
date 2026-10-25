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

// MARK: - 0.2.1 format matrix fixtures

func writeFrames(
    name: String,
    type: UTType,
    frames: [(CGImage, Int)]
) throws {
    let url = outputDirectory.appending(path: name)
    guard let destination = CGImageDestinationCreateWithURL(
        url as CFURL,
        type.identifier as CFString,
        frames.count,
        nil
    ) else {
        throw FixtureError.destinationCreation
    }
    for (image, orientation) in frames {
        let properties: [CFString: Any] = [
            kCGImagePropertyOrientation: orientation,
            kCGImageDestinationLossyCompressionQuality: 0.9,
        ]
        CGImageDestinationAddImage(destination, image, properties as CFDictionary)
    }
    guard CGImageDestinationFinalize(destination) else {
        throw FixtureError.destinationFinalize
    }
}

extension Data {
    mutating func appendLE(_ value: UInt16) {
        append(UInt8(value & 0xff))
        append(UInt8(value >> 8))
    }

    mutating func appendLE(_ value: UInt32) {
        append(UInt8(value & 0xff))
        append(UInt8((value >> 8) & 0xff))
        append(UInt8((value >> 16) & 0xff))
        append(UInt8(value >> 24))
    }

    mutating func appendLE(_ value: Int32) {
        appendLE(UInt32(bitPattern: value))
    }
}

// Minimal single-entry ICO: ICONDIR + ICONDIRENTRY + bottom-up 32bpp BMP body.
func writeIco(name: String, width: Int = 16, height: Int = 16) throws {
    var bmp = Data()
    bmp.appendLE(UInt32(40))
    bmp.appendLE(Int32(width))
    bmp.appendLE(Int32(height * 2))
    bmp.appendLE(UInt16(1))
    bmp.appendLE(UInt16(32))
    bmp.appendLE(UInt32(0))
    bmp.appendLE(UInt32(0))
    bmp.appendLE(Int32(1))
    bmp.appendLE(Int32(1))
    bmp.appendLE(UInt32(0))
    bmp.appendLE(UInt32(0))
    for _ in 0..<(width * height) {
        bmp.append(contentsOf: [0x3c, 0x69, 0xf5, 0xff])
    }
    let maskRowBytes = (width + 31) / 32 * 4
    bmp.append(Data(repeating: 0, count: maskRowBytes * height))

    var ico = Data()
    ico.appendLE(UInt16(0))
    ico.appendLE(UInt16(1))
    ico.appendLE(UInt16(1))
    ico.append(UInt8(width))
    ico.append(UInt8(height))
    ico.append(0)
    ico.append(0)
    ico.appendLE(UInt16(1))
    ico.appendLE(UInt16(32))
    ico.appendLE(UInt32(bmp.count))
    ico.appendLE(UInt32(22))
    ico.append(bmp)
    try ico.write(to: outputDirectory.appending(path: name))
}

let gifFrames = (
    try makeImage(width: 640, height: 480, colorSpace: sRGB, transparent: false),
    try makeImage(width: 640, height: 480, colorSpace: sRGB, transparent: false)
)
try writeFrames(name: "animated.gif", type: .gif, frames: [(gifFrames.0, 1), (gifFrames.1, 1)])
try writeFrames(
    name: "pages.tiff",
    type: .tiff,
    frames: [
        (try makeImage(width: 800, height: 600, colorSpace: sRGB, transparent: false), 1),
        (try makeImage(width: 800, height: 600, colorSpace: sRGB, transparent: false), 1),
    ]
)
try write(
    image: makeImage(width: 512, height: 384, colorSpace: sRGB, transparent: false),
    name: "solid.bmp",
    type: .bmp
)
try write(
    image: makeImage(width: 640, height: 480, colorSpace: displayP3, transparent: false),
    name: "p3.heic",
    type: .heic
)
try write(
    image: makeImage(width: 640, height: 480, colorSpace: sRGB, transparent: false),
    name: "grid.avif",
    type: UTType("public.avif")!
)
try writeIco(name: "dot.ico")

for (source, corrupt) in [
    ("animated.gif", "corrupt.gif"), ("pages.tiff", "corrupt.tiff"),
    ("p3.heic", "corrupt.heic"), ("grid.avif", "corrupt.avif"),
] {
    let data = try Data(contentsOf: outputDirectory.appending(path: source))
    try data.prefix(64).write(to: outputDirectory.appending(path: corrupt))
}

// JPEG bytes under a .webp name: the classifier trusts extensions, the decoder
// must trust content (or refuse), never mis-decode.
try jpegData.write(to: outputDirectory.appending(path: "mismatch.webp"))

// Lying-dimension sample: valid PNG container with a forged IHDR declaring
// 60000x60000. The pipeline must refuse it (corrupt or over budget), never
// decode it as-is.
var forged = try Data(contentsOf: outputDirectory.appending(path: "alpha.png"))
for (offset, value) in [(16, UInt32(60_000)), (20, UInt32(60_000))] {
    forged[offset] = UInt8(value >> 24)
    forged[offset + 1] = UInt8((value >> 16) & 0xff)
    forged[offset + 2] = UInt8((value >> 8) & 0xff)
    forged[offset + 3] = UInt8(value & 0xff)
}
try forged.write(to: outputDirectory.appending(path: "forged.png"))

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
    "animated.gif": FixtureMetadata(
        format: "gif",
        width: 640,
        height: 480,
        orientation: 1,
        hasAlpha: false,
        profile: nil,
        sha256: try checksum("animated.gif")
    ),
    "corrupt.avif": FixtureMetadata(
        format: "avif",
        width: 0,
        height: 0,
        orientation: 1,
        hasAlpha: false,
        profile: nil,
        sha256: try checksum("corrupt.avif")
    ),
    "corrupt.gif": FixtureMetadata(
        format: "gif",
        width: 0,
        height: 0,
        orientation: 1,
        hasAlpha: false,
        profile: nil,
        sha256: try checksum("corrupt.gif")
    ),
    "corrupt.heic": FixtureMetadata(
        format: "heic",
        width: 0,
        height: 0,
        orientation: 1,
        hasAlpha: false,
        profile: nil,
        sha256: try checksum("corrupt.heic")
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
    "corrupt.tiff": FixtureMetadata(
        format: "tiff",
        width: 0,
        height: 0,
        orientation: 1,
        hasAlpha: false,
        profile: nil,
        sha256: try checksum("corrupt.tiff")
    ),
    "dot.ico": FixtureMetadata(
        format: "ico",
        width: 16,
        height: 16,
        orientation: 1,
        hasAlpha: true,
        profile: nil,
        sha256: try checksum("dot.ico")
    ),
    "forged.png": FixtureMetadata(
        format: "png",
        width: 60_000,
        height: 60_000,
        orientation: 1,
        hasAlpha: true,
        profile: "sRGB IEC61966-2.1",
        sha256: try checksum("forged.png")
    ),
    "grid.avif": FixtureMetadata(
        format: "avif",
        width: 640,
        height: 480,
        orientation: 1,
        hasAlpha: false,
        profile: nil,
        sha256: try checksum("grid.avif")
    ),
    "mismatch.webp": FixtureMetadata(
        format: "jpeg",
        width: 1024,
        height: 768,
        orientation: 1,
        hasAlpha: false,
        profile: "sRGB IEC61966-2.1",
        sha256: try checksum("mismatch.webp")
    ),
    "p3.heic": FixtureMetadata(
        format: "heic",
        width: 640,
        height: 480,
        orientation: 1,
        hasAlpha: false,
        profile: "Display P3",
        sha256: try checksum("p3.heic")
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
    "pages.tiff": FixtureMetadata(
        format: "tiff",
        width: 800,
        height: 600,
        orientation: 1,
        hasAlpha: false,
        profile: nil,
        sha256: try checksum("pages.tiff")
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
    "solid.bmp": FixtureMetadata(
        format: "bmp",
        width: 512,
        height: 384,
        orientation: 1,
        hasAlpha: false,
        profile: nil,
        sha256: try checksum("solid.bmp")
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
