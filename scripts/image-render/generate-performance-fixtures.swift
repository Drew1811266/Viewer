#!/usr/bin/env swift

import CoreGraphics
import Foundation
import ImageIO

struct Options {
    let output: URL
}

func parseOptions() throws -> Options {
    var arguments = CommandLine.arguments.dropFirst().makeIterator()
    var output: String?
    while let argument = arguments.next() {
        switch argument {
        case "--output":
            output = arguments.next()
        default:
            throw NSError(
                domain: "ViewerImageRenderFixture",
                code: 2,
                userInfo: [NSLocalizedDescriptionKey: "Unknown argument: \(argument)"]
            )
        }
    }
    guard let output, output.hasPrefix("/") else {
        throw NSError(
            domain: "ViewerImageRenderFixture",
            code: 2,
            userInfo: [NSLocalizedDescriptionKey: "--output requires an absolute path"]
        )
    }
    return Options(output: URL(fileURLWithPath: output))
}

func generateFixture(at output: URL) throws {
    let width = 7_680
    let height = 4_320
    guard
        let colorSpace = CGColorSpace(name: CGColorSpace.sRGB),
        let context = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: width * 4,
            space: colorSpace,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )
    else {
        throw NSError(
            domain: "ViewerImageRenderFixture",
            code: 3,
            userInfo: [NSLocalizedDescriptionKey: "Unable to allocate the deterministic 8K canvas"]
        )
    }

    context.setFillColor(CGColor(red: 0.96, green: 0.97, blue: 0.99, alpha: 1))
    context.fill(CGRect(x: 0, y: 0, width: width, height: height))
    let columns = 48
    let rows = 27
    let cellWidth = CGFloat(width) / CGFloat(columns)
    let cellHeight = CGFloat(height) / CGFloat(rows)
    for row in 0..<rows {
        for column in 0..<columns {
            let seed = (row * columns + column) % 31
            let red = CGFloat((seed * 17 + column * 5) % 255) / 255
            let green = CGFloat((seed * 11 + row * 13) % 255) / 255
            let blue = CGFloat((seed * 7 + column + row * 3) % 255) / 255
            context.setFillColor(CGColor(red: red, green: green, blue: blue, alpha: 1))
            context.fill(
                CGRect(
                    x: CGFloat(column) * cellWidth,
                    y: CGFloat(row) * cellHeight,
                    width: cellWidth + 1,
                    height: cellHeight + 1
                )
            )
        }
    }
    context.setStrokeColor(CGColor(red: 1, green: 1, blue: 1, alpha: 0.8))
    context.setLineWidth(12)
    context.stroke(CGRect(x: 48, y: 48, width: width - 96, height: height - 96))

    guard let image = context.makeImage() else {
        throw NSError(
            domain: "ViewerImageRenderFixture",
            code: 4,
            userInfo: [NSLocalizedDescriptionKey: "Unable to finalize the deterministic 8K image"]
        )
    }
    guard
        let destination = CGImageDestinationCreateWithURL(
            output as CFURL,
            "public.jpeg" as CFString,
            1,
            nil
        )
    else {
        throw NSError(
            domain: "ViewerImageRenderFixture",
            code: 5,
            userInfo: [NSLocalizedDescriptionKey: "Unable to create the JPEG destination"]
        )
    }
    CGImageDestinationAddImage(
        destination,
        image,
        [kCGImageDestinationLossyCompressionQuality: 0.9] as CFDictionary
    )
    guard CGImageDestinationFinalize(destination) else {
        throw NSError(
            domain: "ViewerImageRenderFixture",
            code: 6,
            userInfo: [NSLocalizedDescriptionKey: "Unable to write the deterministic 8K JPEG"]
        )
    }
}

do {
    let options = try parseOptions()
    try generateFixture(at: options.output)
    print(options.output.path)
} catch {
    FileHandle.standardError.write(Data("\(error.localizedDescription)\n".utf8))
    exit(1)
}
