// @pxl/gpu/expo/android/src/main/java/me/plei/pixelet/gpu/PxlGpuModule.kt

// This is the GPU's module on Android (src/mod.ts), the same as
// ios/PxlGpuModule.swift under the same names. It holds the GPU's host
// (Gpu.kt) and the view the guest's surface draws into (SurfaceView.kt).

package me.plei.pixelet.gpu

import android.os.Handler
import android.os.Looper
import android.view.Choreographer
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

class PxlGpuModule : Module() {
  private val main = Handler(Looper.getMainLooper())

  /** This sends the module's event from the GPU's thread, and Expo carries
   * it to the JavaScript thread. */
  private val hear = { s: String -> sendEvent("onState", mapOf("state" to s)) }

  /** This tells the GPU's host of each display frame (`Gpu.vsync`), from the
   * main thread's Choreographer, with how long ago the frame started:
   * `frameTimeNanos` is on `System.nanoTime()`'s clock. */
  private val vsync = object : Choreographer.FrameCallback {
    override fun doFrame(frameTimeNanos: Long) {
      Gpu.vsync(System.nanoTime() - frameTimeNanos)
      Choreographer.getInstance().postFrameCallback(this)
    }
  }

  override fun definition() = ModuleDefinition {
    Name("PxlGpu")

    /** Fires with what the host has of a GPU, as it changes. It is sent from
     * the GPU's thread, and Expo carries the event to the JavaScript thread. */
    Events("onState")

    // The host tells the newest module made, and a module clears the host's
    // callback only while it is its own, under the lock of `Gpu`, as on iOS,
    // where a reload may make the new module before the old one is destroyed.
    OnCreate {
      synchronized(Gpu) { Gpu.hear = hear }
      Gpu.init()
      main.post { Choreographer.getInstance().postFrameCallback(vsync) }
    }

    OnDestroy {
      main.post { Choreographer.getInstance().removeFrameCallback(vsync) }
      synchronized(Gpu) { if (Gpu.hear === hear) Gpu.hear = null }
    }

    /** What the host has of a GPU now, or null before the guest's first ask
     * is answered (`Gpu.state()`). */
    Function("state") { Gpu.state() }

    View(SurfaceView::class) {
      Events("onAttach", "onDetach")
      Prop("surface") { view: SurfaceView, surface: Int -> view.surface = surface }
    }
  }
}
