// @xpute/native/android/src/main/java/xpute/XputeModule.kt

package xpute

import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.Choreographer
import expo.modules.kotlin.jni.JavaScriptFunction
import expo.modules.kotlin.jni.NativeArrayBuffer
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

/**
 * This is xpute's native module on Android (src/mod.ts), the same as
 * ios/XputeModule.swift under the same names. It holds the core (Core.kt), its
 * GPU runtime (Gpu.kt) and the view its surface draws into (SurfaceView.kt).
 */
class XputeModule : Module() {
  private val main = Handler(Looper.getMainLooper())

  /** This tells the core's thread each display frame (`Core.vsync`). */
  private val frames = object : Choreographer.FrameCallback {
    override fun doFrame(frameTimeNanos: Long) {
      Core.vsync((System.nanoTime() - frameTimeNanos) / 1e6)
      Choreographer.getInstance().postFrameCallback(this)
    }
  }

  override fun definition() = ModuleDefinition {
    Name("Xpute")

    /** Fires when the core has posted or asked for a turn, when a call waits
     * for `answer`, or when the core has stopped. It is sent from the core's
     * thread, and Expo carries the event to the JavaScript thread. */
    Events("onNotify")

    OnCreate {
      Core.notify = { sendEvent("onNotify") }
      main.post { Choreographer.getInstance().postFrameCallback(frames) }
    }

    OnDestroy {
      main.post { Choreographer.getInstance().removeFrameCallback(frames) }
      Core.notify = null
      Core.answer = null
    }

    // The region belongs to the core for the app's whole life, so nothing is
    // freed when the buffer is.
    Function("mem") { NativeArrayBuffer(Core.mem()) }

    Function("load") { at: Double -> Core.load(at) }

    Function("store") { at: Double, word: Double -> Core.store(at, word) }

    Function("run") { deadlineMs: Double -> Core.run(deadlineMs) }

    Function("kick") { deadlineMs: Double, atMs: Double -> Core.kick(deadlineMs, atMs) }

    Function("nextFrame") { afterMs: Double -> Core.nextFrame(afterMs) }

    Function("pause") { Core.pause() }

    Function("resume") { Core.resume() }

    Function("answer") { Core.answer() }

    Function("turns") { Core.turns().toList() }

    // @xpute/runtime's answer, called on the JavaScript thread, which is the
    // app's thread here.
    Function("callOn") { answer: JavaScriptFunction<Int> -> Core.answer = { at -> answer.invoke(at) } }

    Function("gpuOpen") { Gpu.open() }

    Function("say") { line: String -> Log.i("xpute", line); Unit }

    View(SurfaceView::class) {
      Events("onAttach", "onDetach")
      Prop("surface") { view: SurfaceView, surface: Int -> view.surface = surface }
    }
  }
}
