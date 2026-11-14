// @pxl/native/modules/platform/ios/PlatformModule.swift

import CoreLocation
import ExpoModulesCore
import ExpoUI
import PxlNative
import UIKit

/// The device's half of the client's services on iOS (the .native files in
/// ts/client/src/service/platform/), as one module that answers what the UI
/// asks of the system. The core is in xpute's own module
/// (@xpute/native-runtime), and the view it draws into in
/// @xpute/gpu-native-runtime's.
public class PlatformModule: Module {
  private static let suite = "pxl"
  private static let storage = UserDefaults(suiteName: suite)!

  public func definition() -> ModuleDefinition {
    Name("PxlPlatform")

    // ---- @expo/ui modifiers the UI names (SheetContent.swift) ----

    OnCreate {
      pxl_text_on(
        { weight, px, text, len in
          autoreleasepool {
            textWidth(String(decoding: UnsafeBufferPointer(start: text, count: len), as: UTF8.self), weight, px)
          }
        },
        { weight, px, rgba, text, len, x, width, height, out in
          autoreleasepool {
            textRaster(
              String(decoding: UnsafeBufferPointer(start: text, count: len), as: UTF8.self), weight, px, rgba, x,
              Int(width), Int(height), UnsafeMutableRawPointer(out!))
          }
        })
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
    // localStorage is the browser's for a page. They are kept in a suite of
    // their own, "pxl", which nothing else in the app writes, as Android keeps
    // them in its "pxl" preferences.

    Function("storage_get") { (key: String) -> String? in
      PlatformModule.storage.string(forKey: key)
    }

    Function("storage_keys") { () -> [String] in
      PlatformModule.storage.persistentDomain(forName: PlatformModule.suite).map { Array($0.keys) } ?? []
    }

    Function("storage_set") { (key: String, value: String) in
      PlatformModule.storage.set(value, forKey: key)
    }

    // ---- the clipboard (service/platform/share.native.ts) ----
    //
    // The general pasteboard serves as navigator.clipboard does on a page.
    // React Native's JavaScript has no clipboard of its own.

    Function("clipboard_set") { (text: String) in
      UIPasteboard.general.string = text
    }

    // ---- the compass (service/platform/sensor.native.ts) ----

    /// Whether the device has a compass at all. expo-location tells that
    /// only by failing a watch, and a failed watch reads as a compass lost
    /// rather than one that was never there.
    Function("has_heading") { () -> Bool in
      CLLocationManager.headingAvailable()
    }

    // ---- the safe area (service/platform/safe-area.native.ts) ----

    /// How much of the screen's bottom inset takes a touch, in points: none,
    /// because the home indicator takes only a swipe.
    Function("tappable_bottom") { () -> Double in
      0
    }

    // ---- the keyboard (ts/client/src/ui/shared/component/above-keyboard.native.tsx, KeyboardDock.swift) ----

    // ---- the world's surface (service/platform/surface.native.tsx) ----

    View(WorldView.self) {
      Events("onAttach", "onDetach", "onPointer")
      Prop("surface") { (view: WorldView, surface: UInt32) in
        view.surface = surface
      }
    }

    View(KeyboardDock.self) {
      Events("onLift")
      Prop("clearance") { (view: KeyboardDock, clearance: Double) in
        view.clearance = clearance
      }
      Prop("rest") { (view: KeyboardDock, rest: Double) in
        view.rest = rest
      }
      Prop("layoutLiftPx") { (view: KeyboardDock, layoutLiftPx: Double) in
        view.layoutLift = layoutLiftPx
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
