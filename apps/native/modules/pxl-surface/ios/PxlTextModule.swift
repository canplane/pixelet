// @pxl/native/modules/pxl-surface/ios/PxlTextModule.swift

import CoreText
import ExpoModulesCore
import UIKit

/// Text in the platform's own family, as the kernel asks for it
/// (apps/native/src/platform/text.ts): how wide a line runs, and a line
/// drawn. The system font, which falls back through every script and draws
/// color emoji as the app's own text does.
public class PxlTextModule: Module {
  public func definition() -> ModuleDefinition {
    Name("PxlText")

    Function("width") { (weight: Double, px: Double, text: String) -> Double in
      Double(CTLineGetTypographicBounds(line(text, weight, px, 0xffffffff), nil, nil, nil))
    }

    /// The line starting at `x` with the middle of its em box on the middle
    /// row, into `out`: `width` by `height` pixels, RGBA, top row first, alpha
    /// not premultiplied.
    Function("raster") { (weight: Double, px: Double, rgba: UInt32, text: String, x: Double, width: Int, height: Int, out: Uint8Array) in
      guard out.byteLength >= width * height * 4,
        let space = CGColorSpace(name: CGColorSpace.sRGB),
        let context = CGContext(
          data: out.rawPointer, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4, space: space,
          bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
      else { return }
      context.clear(CGRect(x: 0, y: 0, width: width, height: height))
      let f = font(weight, px)
      // CoreGraphics counts up from the bottom row.
      context.textPosition = CGPoint(x: x, y: Double(height) / 2 - Double(f.ascender + f.descender) / 2)
      CTLineDraw(line(text, weight, px, rgba), context)
      // CoreGraphics draws premultiplied, and the kernel reads straight alpha.
      let bytes = out.rawPointer.assumingMemoryBound(to: UInt8.self)
      for i in stride(from: 0, to: width * height * 4, by: 4) {
        let a = Int(bytes[i + 3])
        if a == 0 || a == 255 { continue }
        for c in 0..<3 {
          bytes[i + c] = UInt8(min(255, (Int(bytes[i + c]) * 255 + a / 2) / a))
        }
      }
    }
  }
}

/// The system font at a CSS weight (400 regular, 600 semibold) and a size.
private func font(_ weight: Double, _ px: Double) -> UIFont {
  let w: UIFont.Weight =
    switch weight {
    case ..<250: .thin
    case ..<350: .light
    case ..<450: .regular
    case ..<550: .medium
    case ..<650: .semibold
    case ..<750: .bold
    case ..<850: .heavy
    default: .black
    }
  return UIFont.systemFont(ofSize: px, weight: w)
}

private func line(_ text: String, _ weight: Double, _ px: Double, _ rgba: UInt32) -> CTLine {
  let color = CGColor(
    srgbRed: CGFloat(rgba >> 24) / 255, green: CGFloat((rgba >> 16) & 0xff) / 255, blue: CGFloat((rgba >> 8) & 0xff) / 255,
    alpha: CGFloat(rgba & 0xff) / 255)
  let attributes: [NSAttributedString.Key: Any] = [.font: font(weight, px), .foregroundColor: color]
  return CTLineCreateWithAttributedString(NSAttributedString(string: text, attributes: attributes))
}
