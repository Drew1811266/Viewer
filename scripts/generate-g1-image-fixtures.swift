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

// MARK: - 0.2.2 vector fixtures

let logoSvg = """
<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="640" height="480" viewBox="0 0 640 480">
  <rect width="640" height="480" fill="#1f52c8"/>
  <rect x="160" y="120" width="320" height="240" fill="#f55c2c"/>
</svg>

"""
try Data(logoSvg.utf8).write(to: outputDirectory.appending(path: "logo.svg"))

// gzip(logo.svg, level 9) produced once and pinned byte-for-byte.
let logoSvgz: [UInt8] = [31,139,8,0,21,226,221,106,2,255,117,142,77,14,131,32,16,70,247,158,98,50,93,87,16,127,98,26,208,164,139,158,160,61,1,34,144,88,104,148,136,189,125,113,83,187,233,106,50,51,239,123,51,188,223,158,19,172,106,94,172,119,2,139,156,34,40,39,253,96,157,22,248,184,223,206,45,246,93,198,151,85,67,34,221,34,208,132,240,186,16,18,99,204,99,153,251,89,19,70,41,37,137,64,136,118,8,70,96,83,37,141,81,86,155,32,176,106,83,179,90,21,175,126,19,72,129,66,90,195,62,237,50,0,62,43,25,254,231,70,59,77,2,79,197,88,51,217,34,57,18,73,85,52,9,120,167,202,232,247,114,201,126,12,172,58,12,99,93,75,38,119,3,223,95,237,178,15,209,105,97,107,250,0,0,0]
try Data(logoSvgz).write(to: outputDirectory.appending(path: "logo.svgz"))

// Minimal one-page PDF (AI PDF-compatible stream), pinned byte-for-byte.
let posterAi: [UInt8] = [37,80,68,70,45,49,46,52,10,49,32,48,32,111,98,106,60,60,47,84,121,112,101,47,67,97,116,97,108,111,103,47,80,97,103,101,115,32,50,32,48,32,82,62,62,101,110,100,111,98,106,10,50,32,48,32,111,98,106,60,60,47,84,121,112,101,47,80,97,103,101,115,47,75,105,100,115,91,51,32,48,32,82,93,47,67,111,117,110,116,32,49,62,62,101,110,100,111,98,106,10,51,32,48,32,111,98,106,60,60,47,84,121,112,101,47,80,97,103,101,47,80,97,114,101,110,116,32,50,32,48,32,82,47,77,101,100,105,97,66,111,120,91,48,32,48,32,54,52,48,32,52,56,48,93,47,67,111,110,116,101,110,116,115,32,52,32,48,32,82,62,62,101,110,100,111,98,106,10,52,32,48,32,111,98,106,60,60,47,76,101,110,103,116,104,32,50,54,62,62,115,116,114,101,97,109,10,48,32,48,32,49,32,114,103,32,48,32,48,32,54,52,48,32,52,56,48,32,114,101,32,102,10,101,110,100,115,116,114,101,97,109,10,101,110,100,111,98,106,10,120,114,101,102,10,48,32,53,10,48,48,48,48,48,48,48,48,48,48,32,54,53,53,51,53,32,102,32,10,48,48,48,48,48,48,48,48,48,57,32,48,48,48,48,48,32,110,32,10,48,48,48,48,48,48,48,48,53,50,32,48,48,48,48,48,32,110,32,10,48,48,48,48,48,48,48,49,48,49,32,48,48,48,48,48,32,110,32,10,48,48,48,48,48,48,48,49,55,57,32,48,48,48,48,48,32,110,32,10,116,114,97,105,108,101,114,60,60,47,83,105,122,101,32,53,47,82,111,111,116,32,49,32,48,32,82,62,62,10,115,116,97,114,116,120,114,101,102,10,50,53,48,10,37,37,69,79,70,10]
try Data(posterAi).write(to: outputDirectory.appending(path: "poster.ai"))

let legacyAi = "%!PS-Adobe-3.0\n%%BoundingBox: 0 0 640 480\n0 0 1 setrgbcolor\n0 0 640 480 rectfill\nshowpage\n"
try Data(legacyAi.utf8).write(to: outputDirectory.appending(path: "legacy.ai"))

// Deeply nested groups exercise the parser's recursion guard.
let nestedSvg = """
<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="640" height="480" viewBox="0 0 640 480">
\((0..<5000).map { _ in "<g>" }.joined())<rect width="640" height="480" fill="#1f52c8"/>\((0..<5000).map { _ in "</g>" }.joined())</svg>

"""
try Data(nestedSvg.utf8).write(to: outputDirectory.appending(path: "nested.svg"))

let hugeDimensionsSvg = """
<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="60000" height="60000" viewBox="0 0 60000 60000">
  <rect width="60000" height="60000" fill="#1f52c8"/>
</svg>

"""
try Data(hugeDimensionsSvg.utf8).write(to: outputDirectory.appending(path: "huge-dimensions.svg"))

let externalRefSvg = """
<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="640" height="480" viewBox="0 0 640 480">
  <script>alert(1)</script>
  <image xlink:href="file:///etc/passwd" x="0" y="0" width="640" height="480"/>
  <rect width="640" height="480" fill="#1f52c8"/>
</svg>

"""
try Data(externalRefSvg.utf8).write(to: outputDirectory.appending(path: "external-ref.svg"))

for (source, corrupt) in [("logo.svg", "corrupt.svg"), ("poster.ai", "corrupt.ai")] {
    let data = try Data(contentsOf: outputDirectory.appending(path: source))
    try data.prefix(64).write(to: outputDirectory.appending(path: corrupt))
}

// bomb.svgz (gzip of 70 MiB of zeros) is a committed binary sample; the
// manifest below checksums it in place instead of regenerating it.

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
    "bomb.svgz": FixtureMetadata(
        format: "svgz",
        width: 0,
        height: 0,
        orientation: 1,
        hasAlpha: true,
        profile: nil,
        sha256: try checksum("bomb.svgz")
    ),
    "corrupt.ai": FixtureMetadata(
        format: "ai",
        width: 0,
        height: 0,
        orientation: 1,
        hasAlpha: false,
        profile: nil,
        sha256: try checksum("corrupt.ai")
    ),
    "corrupt.svg": FixtureMetadata(
        format: "svg",
        width: 0,
        height: 0,
        orientation: 1,
        hasAlpha: true,
        profile: nil,
        sha256: try checksum("corrupt.svg")
    ),
    "external-ref.svg": FixtureMetadata(
        format: "svg",
        width: 640,
        height: 480,
        orientation: 1,
        hasAlpha: true,
        profile: nil,
        sha256: try checksum("external-ref.svg")
    ),
    "huge-dimensions.svg": FixtureMetadata(
        format: "svg",
        width: 60000,
        height: 60000,
        orientation: 1,
        hasAlpha: true,
        profile: nil,
        sha256: try checksum("huge-dimensions.svg")
    ),
    "legacy.ai": FixtureMetadata(
        format: "postscript-ai",
        width: 640,
        height: 480,
        orientation: 1,
        hasAlpha: false,
        profile: nil,
        sha256: try checksum("legacy.ai")
    ),
    "logo.svg": FixtureMetadata(
        format: "svg",
        width: 640,
        height: 480,
        orientation: 1,
        hasAlpha: true,
        profile: nil,
        sha256: try checksum("logo.svg")
    ),
    "logo.svgz": FixtureMetadata(
        format: "svgz",
        width: 640,
        height: 480,
        orientation: 1,
        hasAlpha: true,
        profile: nil,
        sha256: try checksum("logo.svgz")
    ),
    "nested.svg": FixtureMetadata(
        format: "svg",
        width: 640,
        height: 480,
        orientation: 1,
        hasAlpha: true,
        profile: nil,
        sha256: try checksum("nested.svg")
    ),
    "poster.ai": FixtureMetadata(
        format: "ai",
        width: 640,
        height: 480,
        orientation: 1,
        hasAlpha: false,
        profile: nil,
        sha256: try checksum("poster.ai")
    ),
])

let encoder = JSONEncoder()
encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
var manifestData = try encoder.encode(manifest)
manifestData.append(0x0A)
try manifestData.write(to: outputDirectory.appending(path: "manifest.json"))
