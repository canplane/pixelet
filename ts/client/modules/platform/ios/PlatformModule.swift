// @pxl/native/modules/platform/ios/PlatformModule.swift

import CoreLocation
import ExpoModulesCore
import ExpoUI
import PxlNative
import UIKit

/// The device's half of the client's services (ts/client/src/service/, the
/// .native files), as one module: what the shell asks of the system. The kernel and the view it
/// draws into are xpute's module (@xpute/native). An image's decode runs off
/// the JavaScript thread.
public class PlatformModule: Module {

  public func definition() -> ModuleDefinition {
    Name("PxlPlatform")

    // ---- @expo/ui modifiers the shell names (SheetContent.swift) ----

    OnCreate {
      ViewModifierRegistry.register(contentScrollsModifier) { _, _, _ in ContentScrollsModifier() }
    }
    OnDestroy {
      ViewModifierRegistry.unregister(contentScrollsModifier)
    }

    // ---- the transport (service/origin.native.ts) ----

    /// A body fetched still zstd, decoded off the JavaScript thread on the
    /// module's queue, into the library's bytes, which JavaScript takes
    /// without a copy and the library lets go with the buffer.
    AsyncFunction("unpack") { (packed: Data) throws -> NativeArrayBuffer in
      var out: UnsafeMutablePointer<UInt8>?
      var count = 0
      let decoded = packed.withUnsafeBytes { (src: UnsafeRawBufferPointer) in
        pxl_unpack(src.bindMemory(to: UInt8.self).baseAddress, src.count, &out, &count)
      }
      guard decoded == 0, let bytes = out else { throw UnpackException() }
      return try NativeArrayBuffer.wrap(dataWithoutCopy: UnsafeMutableRawBufferPointer(start: bytes, count: count), cleanup: { pxl_unpacked_free(bytes, count) })
    }

    // ---- storage (service/storage.native.ts) ----
    //
    // UserDefaults, the system's store for an app's small values, as a page's
    // localStorage is the browser's.

    Function("storageGet") { (key: String) -> String? in
      UserDefaults.standard.string(forKey: key)
    }

    Function("storageSet") { (key: String, value: String) in
      UserDefaults.standard.set(value, forKey: key)
    }

    // ---- the clipboard (service/share.native.ts) ----
    //
    // The general pasteboard, as a page's navigator.clipboard is the
    // browser's; React Native's JavaScript has none of its own.

    Function("clipboardSet") { (text: String) in
      UIPasteboard.general.string = text
    }

    // ---- text (service/text.native.ts, Text.swift) ----


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

    // ---- the compass (service/sensor.native.ts) ----

    /// Whether the device has a compass at all, which expo-horizon-location only says
    /// by failing a watch — and a failed watch reads as a compass lost, not
    /// one never there.
    Function("headingAvailable") { () -> Bool in
      CLLocationManager.headingAvailable()
    }

    // ---- the keyboard (ts/client/src/shell/ui/above-keyboard.native.tsx, KeyboardDock.swift) ----
    //
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

    // ---- the segmented control (ts/client/src/shell/ui/segmented.ios.tsx, SegmentedControl.swift) ----

    View(SegmentedControl.self) {
      Events("onPick")
      Prop("labels") { (view: SegmentedControl, labels: [String]) in
        view.labels = labels
      }
      Prop("selectedIndex") { (view: SegmentedControl, index: Int) in
        view.selectedIndex = index
      }
    }

    // ---- a see-through backing's blur (ts/client/src/shell/ui/backing.ios.tsx, Blur.swift) ----

    View(Blur.self) {}
  }
}

internal final class UnpackException: Exception, @unchecked Sendable {
  override var reason: String {
    "a body sent zstd does not decode"
  }
}
