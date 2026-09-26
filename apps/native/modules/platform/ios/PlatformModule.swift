// @pxl/native/modules/platform/ios/PlatformModule.swift

import CoreLocation
import ExpoModulesCore
import PxlNative
import UIKit

/// The device's half of the app's platform (apps/native/src/platform/), as
/// one module: the kernel as the shell reaches it, the view it draws into,
/// and what the shell asks of the system.
///
/// The kernel's memory is handed over as one ArrayBuffer over the region the
/// core took — no copy, so a ring the shell writes is the ring the kernel
/// reads. The shell calls on the JavaScript thread, one call at a time; the
/// core runs each on its own thread while this one waits
/// (crates/pxl/web/native/src/thread.rs), and an image's decode runs off both.
public class PlatformModule: Module {
  public func definition() -> ModuleDefinition {
    Name("PxlPlatform")

    // ---- the kernel (platform/core.ts, gpu.ts, image.ts) ----

    Function("memory") { () throws -> NativeArrayBuffer in
      var bytes: UInt32 = 0
      guard let at = pxl_memory(&bytes) else { throw MemoryException() }
      // The region is the core's for the app's life: nothing is let go when
      // the buffer is.
      return try NativeArrayBuffer.wrap(dataWithoutCopy: UnsafeMutableRawBufferPointer(start: at, count: Int(bytes)), cleanup: {})
    }

    Function("interrupt") { (quotaMs: Double) -> Double in
      pxl_interrupt(quotaMs)
    }

    /// The device's uniform offset alignment and whether it times its
    /// passes, or nil where the system has no GPU.
    Function("gpuOpen") { () -> [UInt32]? in
      var align: UInt32 = 0
      var timestamps: UInt32 = 0
      return pxl_gpu_open(&align, &timestamps) == 0 ? [align, timestamps] : nil
    }

    /// Decodes a PNG or a WebP and holds it for the renderer: its handle, or 0
    /// when it does not decode. Async, so it runs on the module's queue rather
    /// than the JavaScript thread: a large image costs no turn.
    AsyncFunction("imageHold") { (file: Data) -> UInt32 in
      file.withUnsafeBytes { bytes in
        pxl_image_hold(bytes.bindMemory(to: UInt8.self).baseAddress, UInt32(bytes.count))
      }
    }

    // ---- storage (platform/storage.ts) ----
    //
    // UserDefaults, the system's store for an app's small values, as a page's
    // localStorage is the browser's.

    Function("storageGet") { (key: String) -> String? in
      UserDefaults.standard.string(forKey: key)
    }

    Function("storageSet") { (key: String, value: String) in
      UserDefaults.standard.set(value, forKey: key)
    }

    // ---- the clipboard (platform/save.ts) ----
    //
    // The general pasteboard, as a page's navigator.clipboard is the
    // browser's; React Native's JavaScript has none of its own.

    Function("clipboardSet") { (text: String) in
      UIPasteboard.general.string = text
    }

    // ---- text (platform/text.ts, Text.swift) ----

    Function("textWidth") { (weight: Double, px: Double, text: String) -> Double in
      textWidth(text, weight, px)
    }

    /// The line starting at `x` with the middle of its em box on the middle
    /// row, into `out`: `width` by `height` pixels, RGBA, top row first, alpha
    /// not premultiplied.
    Function("textRaster") { (weight: Double, px: Double, rgba: UInt32, text: String, x: Double, width: Int, height: Int, out: Uint8Array) in
      guard out.byteLength >= width * height * 4 else { return }
      textRaster(text, weight, px, rgba, x, width, height, out.rawPointer)
    }

    // ---- the compass (platform/sensor.ts) ----

    /// Whether the device has a compass at all, which expo-location only says
    /// by failing a watch — and a failed watch reads as a compass lost, not
    /// one never there.
    Function("headingAvailable") { () -> Bool in
      CLLocationManager.headingAvailable()
    }

    // ---- the surface (platform/surface.tsx, SurfaceView.swift) ----

    View(SurfaceView.self) {
      Events("onAttach", "onDetach")
      Prop("surface") { (view: SurfaceView, surface: UInt32) in
        view.surface = surface
      }
    }
  }
}

internal final class MemoryException: Exception, @unchecked Sendable {
  override var reason: String {
    "the core has no memory"
  }
}
