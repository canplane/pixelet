// @xpute/gpu-native-runtime/ios/XputeGpuModule.swift

/// This is the GPU's module on iOS (src/mod.ts), over its C ABI
/// (xpute_gpu.h), with the view the guest's surface draws into
/// (SurfaceView.swift). android/'s XputeGpuModule.kt is the same module under
/// the same names.
import CXputeGpu
import ExpoModulesCore
import QuartzCore

/// This tells the GPU's host of each display frame (`xpute_gpu_vsync`), from a
/// display link on the main thread.
final class Vsync: NSObject {
  private var link: CADisplayLink?

  func start() {
    DispatchQueue.main.async {
      self.link = CADisplayLink(target: self, selector: #selector(self.frame))
      self.link?.add(to: .main, forMode: .common)
    }
  }

  func stop() {
    DispatchQueue.main.async {
      self.link?.invalidate()
      self.link = nil
    }
  }

  @objc private func frame() {
    xpute_gpu_vsync()
  }
}

public class XputeGpuModule: Module {
  /// The function given to `xpute_gpu_state_on` sends its event through this
  /// module, the newest made. A reload may make the new module before the old
  /// one is destroyed, so a module clears the function only while it is the
  /// current one, under `lock`.
  nonisolated(unsafe) static weak var current: XputeGpuModule?
  private static let lock = NSLock()
  private let vsync = Vsync()

  public func definition() -> ModuleDefinition {
    Name("XputeGpu")

    /// Fires with what the host has of a GPU, as it changes.
    Events("onState")

    OnCreate {
      XputeGpuModule.lock.lock()
      defer { XputeGpuModule.lock.unlock() }
      XputeGpuModule.current = self
      // A C function captures nothing, so it reaches the module through the
      // static. It is called on the GPU's thread, and Expo's sendEvent
      // carries the event to the JavaScript thread.
      xpute_gpu_state_on { state, len in
        XputeGpuModule.current?.sendEvent(
          "onState", ["state": String(decoding: UnsafeRawBufferPointer(start: state, count: len), as: UTF8.self)])
      }
      self.vsync.start()
    }
    OnDestroy {
      self.vsync.stop()
      XputeGpuModule.lock.lock()
      defer { XputeGpuModule.lock.unlock() }
      guard XputeGpuModule.current === self else { return }
      XputeGpuModule.current = nil
      xpute_gpu_state_on(nil)
    }

    View(SurfaceView.self) {
      Events("onAttach", "onDetach")
      Prop("surface") { (view: SurfaceView, surface: UInt32) in
        view.surface = surface
      }
    }
  }
}
