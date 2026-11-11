// @xpute/gpu/ios/XputeGpuModule.swift

/// This is the GPU's module on iOS (src/native.ts), over its C ABI
/// (xpute_gpu.h), with the view the core's surface draws into
/// (SurfaceView.swift). android/'s XputeGpuModule.kt is the same module under
/// the same names.
import CXputeGpu
import ExpoModulesCore

public class XputeGpuModule: Module {
  /// The function given to `xpute_gpu_lost_on` sends its event through this
  /// module.
  nonisolated(unsafe) static weak var current: XputeGpuModule?

  public func definition() -> ModuleDefinition {
    Name("XputeGpu")

    /// Fires once each time the open device is lost and let go.
    Events("onLost")

    OnCreate {
      XputeGpuModule.current = self
      // A C function captures nothing, so it reaches the module through the
      // static. It is called from the thread that found the loss, and Expo's
      // sendEvent itself schedules the event onto the JavaScript thread.
      xpute_gpu_lost_on { XputeGpuModule.current?.sendEvent("onLost") }
    }
    OnDestroy {
      XputeGpuModule.current = nil
      xpute_gpu_lost_on(nil)
    }

    // The device keeps its compiled pipelines among the app's caches.
    Function("open") { () -> Bool in
      let caches = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first
      return xpute_gpu_open(caches?.path) == 0
    }

    View(SurfaceView.self) {
      Events("onAttach", "onDetach")
      Prop("surface") { (view: SurfaceView, surface: UInt32) in
        view.surface = surface
      }
    }
  }
}
