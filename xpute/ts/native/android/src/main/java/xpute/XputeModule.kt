// @xpute/native/android/src/main/java/xpute/XputeModule.kt

package xpute

import expo.modules.kotlin.jni.JavaScriptFunction
import expo.modules.kotlin.jni.NativeArrayBuffer
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

/**
 * xpute's module on Android (src/mod.ts), ios/XputeModule.swift's under the
 * same names: the guest (Guest.kt), its GPU host (Gpu.kt) and the view its
 * surface draws into (SurfaceView.kt).
 */
class XputeModule : Module() {
  override fun definition() = ModuleDefinition {
    Name("Xpute")

    /** The guest has posted or asked, a hostcall waits for `answer`, or the
     * guest has stopped. Sent from the guest's thread; Expo carries the
     * event to the JavaScript thread. */
    Events("onNotify")

    OnCreate { Guest.notify = { sendEvent("onNotify") } }

    OnDestroy {
      Guest.notify = null
      Guest.answer = null
    }

    // The region is the guest's for the app's life: nothing is let go when
    // the buffer is.
    Function("mem") { NativeArrayBuffer(Guest.mem()) }

    Function("run") { quotaMs: Double -> Guest.run(quotaMs) }

    Function("kick") { quotaMs: Double -> Guest.kick(quotaMs) }

    Function("answer") { Guest.answer() }

    Function("heard") { Guest.heard().toList() }

    Function("hostcallOn") { answer: JavaScriptFunction<Int> -> Guest.answer = answer }

    Function("gpuOpen") { Gpu.open() }

    View(SurfaceView::class) {
      Events("onAttach", "onDetach")
      Prop("surface") { view: SurfaceView, surface: Int -> view.surface = surface }
    }
  }
}
