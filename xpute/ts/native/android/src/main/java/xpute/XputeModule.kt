// @xpute/native/android/src/main/java/xpute/XputeModule.kt

package xpute

import expo.modules.kotlin.jni.JavaScriptFunction
import expo.modules.kotlin.jni.NativeArrayBuffer
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

/**
 * xpute's module on Android (src/mod.ts), ios/XputeModule.swift's under the
 * same names: the core (Core.kt), its GPU runtime (Gpu.kt) and the view its
 * surface draws into (SurfaceView.kt).
 */
class XputeModule : Module() {
  override fun definition() = ModuleDefinition {
    Name("Xpute")

    /** The core has posted or asked, a call waits for `answer`, or the
     * core has stopped. Sent from the core's thread; Expo carries the
     * event to the JavaScript thread. */
    Events("onNotify")

    OnCreate { Core.notify = { sendEvent("onNotify") } }

    OnDestroy {
      Core.notify = null
      Core.answer = null
    }

    // The region is the core's for the app's life: nothing is let go when
    // the buffer is.
    Function("mem") { NativeArrayBuffer(Core.mem()) }

    Function("run") { deadlineMs: Double -> Core.run(deadlineMs) }

    Function("kick") { deadlineMs: Double -> Core.kick(deadlineMs) }

    Function("pause") { Core.pause() }

    Function("resume") { Core.resume() }

    Function("answer") { Core.answer() }

    Function("turns") { Core.turns().toList() }

    Function("callOn") { answer: JavaScriptFunction<Int> -> Core.answer = answer }

    Function("gpuOpen") { Gpu.open() }

    View(SurfaceView::class) {
      Events("onAttach", "onDetach")
      Prop("surface") { view: SurfaceView, surface: Int -> view.surface = surface }
    }
  }
}
