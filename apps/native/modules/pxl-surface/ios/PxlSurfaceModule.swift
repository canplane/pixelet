// @pxl/native/modules/pxl-surface/ios/PxlSurfaceModule.swift

import ExpoModulesCore

/// The surface as the shell mounts it: `requireNativeView("PxlSurface")`.
public class PxlSurfaceModule: Module {
  public func definition() -> ModuleDefinition {
    Name("PxlSurface")
    View(PxlSurfaceView.self) {
      Events("onAttach", "onDetach")
      Prop("surface") { (view: PxlSurfaceView, surface: UInt32) in
        view.surface = surface
      }
    }
  }
}
