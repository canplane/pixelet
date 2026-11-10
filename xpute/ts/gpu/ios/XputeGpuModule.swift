// @xpute/gpu/ios/XputeGpuModule.swift

/// This is the GPU's module on iOS (src/native.ts), over its C ABI
/// (xpute_gpu.h), with the view the core's surface draws into
/// (SurfaceView.swift). android/'s XputeGpuModule.kt is the same module under
/// the same names.
import CXputeGpu
import ExpoModulesCore

public class XputeGpuModule: Module {
  public func definition() -> ModuleDefinition {
    Name("XputeGpu")

    Function("open") { () -> Bool in
      xpute_gpu_open() == 0
    }

    View(SurfaceView.self) {
      Events("onAttach", "onDetach")
      Prop("surface") { (view: SurfaceView, surface: UInt32) in
        view.surface = surface
      }
    }
  }
}
