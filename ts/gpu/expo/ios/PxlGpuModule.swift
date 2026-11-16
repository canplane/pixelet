// @pxl/gpu/expo/ios/PxlGpuModule.swift

/// This is the GPU's module on iOS (src/mod.ts), over its C ABI
/// (pxl_gpu.h), with the view the guest's surface draws into
/// (SurfaceView.swift). android/'s PxlGpuModule.kt is the same module under
/// the same names.
import CPxlGpu
import ExpoModulesCore
import QuartzCore

/// This tells the GPU's host of each display frame (`pxl_gpu_vsync`), from a
/// display link on the main thread, with how long ago the frame started: the
/// link's `timestamp` is on `CACurrentMediaTime()`'s clock.
final class Vsync: NSObject {
  private var link: CADisplayLink?

  func start() {
    DispatchQueue.main.async {
      self.link = CADisplayLink(target: self, selector: #selector(self.frame(_:)))
      self.link?.add(to: .main, forMode: .common)
    }
  }

  func stop() {
    DispatchQueue.main.async {
      self.link?.invalidate()
      self.link = nil
    }
  }

  @objc private func frame(_ link: CADisplayLink) {
    pxl_gpu_vsync(UInt64(max(0, (CACurrentMediaTime() - link.timestamp) * 1e9)))
  }
}

public class PxlGpuModule: Module {
  /// The function given to `pxl_gpu_state_on` sends its event through this
  /// module, the newest made. A reload may make the new module before the old
  /// one is destroyed, so a module clears the function only while it is the
  /// current one, under `lock`.
  nonisolated(unsafe) static weak var current: PxlGpuModule?
  private static let lock = NSLock()
  private let vsync = Vsync()

  public func definition() -> ModuleDefinition {
    Name("PxlGpu")

    /// Fires with what the host has of a GPU, as it changes.
    Events("onState")

    OnCreate {
      PxlGpuModule.lock.lock()
      defer { PxlGpuModule.lock.unlock() }
      PxlGpuModule.current = self
      // A C function captures nothing, so it reaches the module through the
      // static. It is called on the GPU's thread, and Expo's sendEvent
      // carries the event to the JavaScript thread.
      pxl_gpu_state_on { state, len in
        PxlGpuModule.current?.sendEvent(
          "onState", ["state": String(decoding: UnsafeRawBufferPointer(start: state, count: len), as: UTF8.self)])
      }
      self.vsync.start()
    }
    OnDestroy {
      self.vsync.stop()
      PxlGpuModule.lock.lock()
      defer { PxlGpuModule.lock.unlock() }
      guard PxlGpuModule.current === self else { return }
      PxlGpuModule.current = nil
      pxl_gpu_state_on(nil)
    }

    /// What the host has of a GPU now, or nil before the guest's first ask is
    /// answered (pxl_gpu.h `pxl_gpu_state`).
    Function("state") { () -> String? in
      var len = 0
      guard let state = pxl_gpu_state(&len) else { return nil }
      return String(decoding: UnsafeRawBufferPointer(start: state, count: len), as: UTF8.self)
    }

    View(SurfaceView.self) {
      Events("onAttach", "onDetach")
      Prop("surface") { (view: SurfaceView, surface: UInt32) in
        view.surface = surface
      }
    }
  }
}
