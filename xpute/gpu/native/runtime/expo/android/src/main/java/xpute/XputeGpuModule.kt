// @xpute/gpu-native-runtime/android/src/main/java/xpute/XputeGpuModule.kt

// This is the GPU's module on Android (src/mod.ts), the same as
// ios/XputeGpuModule.swift under the same names. It holds the GPU's host
// (Gpu.kt) and the view the guest's surface draws into (SurfaceView.kt).

package xpute

import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

class XputeGpuModule : Module() {
  override fun definition() = ModuleDefinition {
    Name("XputeGpu")

    /** Fires once each time the open device is lost and let go. It is sent
     * from the thread that found the loss, and Expo carries the event to the
     * JavaScript thread. */
    Events("onLost")

    OnCreate { Gpu.lost = { sendEvent("onLost") } }

    OnDestroy { Gpu.lost = null }

    // The device keeps its compiled pipelines among the app's caches.
    Function("open") { Gpu.open(appContext.cacheDirectory.path) }

    View(SurfaceView::class) {
      Events("onAttach", "onDetach")
      Prop("surface") { view: SurfaceView, surface: Int -> view.surface = surface }
    }
  }
}
