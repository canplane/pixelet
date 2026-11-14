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
  /// The function given to `xpute_gpu_lost_on` sends its event through this
  /// module.
  nonisolated(unsafe) static weak var current: XputeGpuModule?
  private let vsync = Vsync()

  public func definition() -> ModuleDefinition {
    Name("XputeGpu")

    /// Fires once each time a device is lost and let go.
    Events("onLost")

    OnCreate {
      XputeGpuModule.current = self
      // A C function captures nothing, so it reaches the module through the
      // static. It is called on the GPU's thread, and Expo's sendEvent
      // carries the event to the JavaScript thread.
      xpute_gpu_lost_on { XputeGpuModule.current?.sendEvent("onLost") }
      self.vsync.start()
    }
    OnDestroy {
      self.vsync.stop()
      XputeGpuModule.current = nil
      xpute_gpu_lost_on(nil)
    }

    View(SurfaceView.self) {
      Events("onAttach", "onDetach")
      Prop("surface") { (view: SurfaceView, surface: UInt32) in
        view.surface = surface
      }
    }
  }
}
