// @xpute/gpu/android/src/main/java/xpute/XputeGpuModule.kt

// This is the GPU's module on Android (src/native.ts), the same as
// ios/XputeGpuModule.swift under the same names. It holds the GPU's runtime
// (Gpu.kt) and the view the core's surface draws into (SurfaceView.kt).

package xpute

import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

class XputeGpuModule : Module() {
  override fun definition() = ModuleDefinition {
    Name("XputeGpu")

    Function("open") { Gpu.open() }

    View(SurfaceView::class) {
      Events("onAttach", "onDetach")
      Prop("surface") { view: SurfaceView, surface: Int -> view.surface = surface }
    }
  }
}
