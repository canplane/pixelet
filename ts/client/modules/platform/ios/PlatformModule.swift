// @pxl/native/modules/platform/ios/PlatformModule.swift

import CoreLocation
import ExpoModulesCore
import ExpoUI
import PxlNative
import UIKit

/// The device's half of the client's services on iOS (the .native files in
/// ts/client/src/service/platform/), as one module that answers what the UI
/// asks of the system. The core, and the view it draws into, are in xpute's own
/// module (@xpute/native).
public class PlatformModule: Module {

  public func definition() -> ModuleDefinition {
    Name("PxlPlatform")

    // ---- @expo/ui modifiers the UI names (SheetContent.swift) ----

    OnCreate {
      ViewModifierRegistry.register(contentScrollsModifier) { _, _, _ in ContentScrollsModifier() }
    }
    OnDestroy {
      ViewModifierRegistry.unregister(contentScrollsModifier)
    }

    // ---- the transport (service/platform/origin.native.ts) ----

    /// Decodes a body that arrived still zstd-compressed. It runs on the
    /// module's queue, off the JavaScript thread. The bytes are the library's:
    /// JavaScript takes them without a copy, and the library frees them when
    /// the buffer goes.
    AsyncFunction("unpack") { (packed: Data) throws -> NativeArrayBuffer in
      var out: UnsafeMutablePointer<UInt8>?
      var count = 0
      let decoded = packed.withUnsafeBytes { (src: UnsafeRawBufferPointer) in
        pxl_unpack(src.bindMemory(to: UInt8.self).baseAddress, src.count, &out, &count)
      }
      guard decoded == 0, let bytes = out else { throw UnpackException() }
      return try NativeArrayBuffer.wrap(dataWithoutCopy: UnsafeMutableRawBufferPointer(start: bytes, count: count), cleanup: { pxl_unpacked_free(bytes, count) })
    }

    // ---- storage (service/platform/storage.native.ts) ----
    //
    // UserDefaults is the system's store for an app's small values, as
    // localStorage is the browser's for a page.

    Function("storageGet") { (key: String) -> String? in
      UserDefaults.standard.string(forKey: key)
    }

    Function("storageSet") { (key: String, value: String) in
      UserDefaults.standard.set(value, forKey: key)
    }

    // ---- the clipboard (service/platform/share.native.ts) ----
    //
    // The general pasteboard serves as navigator.clipboard does on a page.
    // React Native's JavaScript has no clipboard of its own.

    Function("clipboardSet") { (text: String) in
      UIPasteboard.general.string = text
    }

    // ---- text (service/platform/text.native.ts, Text.swift) ----


    Function("textWidth") { (weight: Double, px: Double, text: String) -> Double in
      textWidth(text, weight, px)
    }

    /// Draws the line into `out`, starting at `x`, with the middle of its em box
    /// on the middle row. `out` is `width` by `height` pixels of RGBA, top row
    /// first, with alpha not premultiplied. An `out` too small for that is left
    /// untouched.
    Function("textRaster") { (weight: Double, px: Double, rgba: UInt32, text: String, x: Double, width: Int, height: Int, out: Uint8Array) in
      guard out.byteLength >= width * height * 4 else { return }
      textRaster(text, weight, px, rgba, x, width, height, out.rawPointer)
    }

    // ---- the compass (service/platform/sensor.native.ts) ----

    /// Whether the device has a compass at all. expo-horizon-location tells that
    /// only by failing a watch, and a failed watch reads as a compass lost
    /// rather than one that was never there.
    Function("headingAvailable") { () -> Bool in
      CLLocationManager.headingAvailable()
    }

    // ---- the keyboard (ts/client/src/ui/shared/component/above-keyboard.native.tsx, KeyboardDock.swift) ----

    View(KeyboardDock.self) {
      Events("onLift")
      Prop("clearance") { (view: KeyboardDock, clearance: Double) in
        view.clearance = clearance
      }
      Prop("rest") { (view: KeyboardDock, rest: Double) in
        view.rest = rest
      }
      Prop("layoutLift") { (view: KeyboardDock, layoutLift: Double) in
        view.layoutLift = layoutLift
      }
    }

    // ---- the segmented control (ts/client/src/ui/shared/component/segmented.ios.tsx, SegmentedControl.swift) ----

    View(SegmentedControl.self) {
      Events("onPick")
      Prop("labels") { (view: SegmentedControl, labels: [String]) in
        view.labels = labels
      }
      Prop("selectedIndex") { (view: SegmentedControl, index: Int) in
        view.selectedIndex = index
      }
    }

    // ---- a see-through backing's blur (ts/client/src/ui/shared/component/backing.ios.tsx, Blur.swift) ----

    View(Blur.self) {}
  }
}

internal final class UnpackException: Exception, @unchecked Sendable {
  override var reason: String {
    "a body sent zstd does not decode"
  }
}
