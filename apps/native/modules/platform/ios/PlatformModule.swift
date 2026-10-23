// @pxl/native/modules/platform/ios/PlatformModule.swift

import CoreLocation
import ExpoModulesCore
import ExpoUI
import PxlNative
import UIKit

/// The device's half of the app's platform (apps/native/src/platform/), as
/// one module: the kernel as the shell reaches it, the view it draws into,
/// and what the shell asks of the system.
///
/// The kernel's memory is handed over as one ArrayBuffer over the region the
/// guest took — no copy, so a ring the shell writes is the ring the kernel
/// reads. The shell calls on the JavaScript thread, one call at a time; the
/// guest runs each on its own thread while this one waits
/// (crates/client-bracket/native/src/thread.rs), and an image's decode runs off both.
public class PlatformModule: Module {
  /// The module `pxl_wake_on`'s function sends through.
  nonisolated(unsafe) static weak var current: PlatformModule?

  public func definition() -> ModuleDefinition {
    Name("PxlPlatform")

    // ---- @expo/ui modifiers the shell names (SheetContent.swift) ----

    OnCreate {
      ViewModifierRegistry.register(contentScrollsModifier) { _, _, _ in ContentScrollsModifier() }
      PlatformModule.current = self
      // A C function captures nothing, so it reaches the module through the
      // static. Called from the thread that learned of a completion: Expo's
      // sendEvent schedules the event onto the JavaScript thread itself.
      pxl_wake_on { PlatformModule.current?.sendEvent("onWake") }
    }
    OnDestroy {
      ViewModifierRegistry.unregister(contentScrollsModifier)
      PlatformModule.current = nil
    }

    // ---- the kernel's turns (platform/gpu.ts) ----

    /// The device's host asks a turn: a completion it writes at the next
    /// turn's poll would otherwise wait for an input to be read.
    Events("onWake")

    // ---- the kernel (platform/guest.ts, gpu.ts) ----

    Function("memory") { () throws -> NativeArrayBuffer in
      var bytes: UInt32 = 0
      guard let at = pxl_memory(&bytes) else { throw MemoryException() }
      // The region is the guest's for the app's life: nothing is let go when
      // the buffer is.
      return try NativeArrayBuffer.wrap(dataWithoutCopy: UnsafeMutableRawBufferPointer(start: at, count: Int(bytes)), cleanup: {})
    }

    Function("interrupt") { (quotaMs: Double) -> Double in
      pxl_interrupt(quotaMs)
    }

    /// Whether the device opened; what it is, the kernel reads from the host.
    Function("gpuOpen") { () -> Bool in
      pxl_gpu_open() == 0
    }

    // ---- the transport (platform/origin.ts) ----

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

    /// Whether the device has a compass at all, which expo-horizon-location only says
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

    // ---- the keyboard (platform/above-keyboard.tsx, KeyboardDock.swift) ----
    //
    // The surface stays the first view, the one the module's name alone
    // reaches; this one is asked for by its own.

    View(KeyboardDock.self) {
      Events("onLift")
      Prop("clearance") { (view: KeyboardDock, clearance: Double) in
        view.clearance = clearance
      }
      Prop("rest") { (view: KeyboardDock, rest: Double) in
        view.rest = rest
      }
      Prop("applied") { (view: KeyboardDock, applied: Double) in
        view.applied = applied
      }
    }

    // ---- the segmented control (platform/segmented.tsx, SegmentedControl.swift) ----

    View(SegmentedControl.self) {
      Events("onPick")
      Prop("labels") { (view: SegmentedControl, labels: [String]) in
        view.labels = labels
      }
      Prop("selectedIndex") { (view: SegmentedControl, index: Int) in
        view.selectedIndex = index
      }
    }

    // ---- a see-through backing's blur (platform/backing.tsx, Blur.swift) ----

    View(Blur.self) {}
  }
}

internal final class MemoryException: Exception, @unchecked Sendable {
  override var reason: String {
    "the guest has no memory"
  }
}

internal final class UnpackException: Exception, @unchecked Sendable {
  override var reason: String {
    "a body sent zstd does not decode"
  }
}
