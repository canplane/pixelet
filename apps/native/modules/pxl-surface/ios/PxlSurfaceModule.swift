// @pxl/native/modules/pxl-surface/ios/PxlSurfaceModule.swift

import ExpoModulesCore

/// The surface as the shell mounts it: `requireNativeView("PxlSurface")`.
public class PxlSurfaceModule: Module {
  public func definition() -> ModuleDefinition {
    Name("PxlSurface")
    View(PxlSurfaceView.self) {}
  }
}
