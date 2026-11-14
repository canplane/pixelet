// @xpute/gpu-native-runtime/android/src/main/java/xpute/XputeGpuModule.kt

// This is the GPU's module on Android (src/mod.ts), the same as
// ios/XputeGpuModule.swift under the same names. It holds the GPU's host
// (Gpu.kt) and the view the guest's surface draws into (SurfaceView.kt).

package xpute

import android.os.Handler
import android.os.Looper
import android.view.Choreographer
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

class XputeGpuModule : Module() {
  private val main = Handler(Looper.getMainLooper())

  /** This sends the module's event from the GPU's thread, and Expo carries
   * it to the JavaScript thread. */
  private val state = { s: String -> sendEvent("onState", mapOf("state" to s)) }

  /** This tells the GPU's host of each display frame (`Gpu.vsync`), from the
   * main thread's Choreographer. */
  private val vsync = object : Choreographer.FrameCallback {
    override fun doFrame(frameTimeNanos: Long) {
      Gpu.vsync()
      Choreographer.getInstance().postFrameCallback(this)
    }
  }

  override fun definition() = ModuleDefinition {
    Name("XputeGpu")

    /** Fires with what the host has of a GPU, as it changes. It is sent from
     * the GPU's thread, and Expo carries the event to the JavaScript thread. */
    Events("onState")

    // The host tells the newest module made, and a module clears the host's
    // callback only while it is its own, under the lock of `Gpu`, as on iOS,
    // where a reload may make the new module before the old one is destroyed.
    OnCreate {
      synchronized(Gpu) { Gpu.state = state }
      Gpu.init()
      main.post { Choreographer.getInstance().postFrameCallback(vsync) }
    }

    OnDestroy {
      main.post { Choreographer.getInstance().removeFrameCallback(vsync) }
      synchronized(Gpu) { if (Gpu.state === state) Gpu.state = null }
    }

    View(SurfaceView::class) {
      Events("onAttach", "onDetach")
      Prop("surface") { view: SurfaceView, surface: Int -> view.surface = surface }
    }
  }
}
