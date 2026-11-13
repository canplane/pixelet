// @xpute/native-runtime/android/src/main/java/xpute/XputeModule.kt

package xpute

import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.Choreographer
import expo.modules.kotlin.jni.JavaScriptFunction
import expo.modules.kotlin.jni.NativeArrayBuffer
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import expo.modules.kotlin.typedarray.Uint8Array
import java.nio.ByteBuffer

/**
 * This is xpute's native module on Android (src/mod.ts), the same as
 * ios/XputeModule.swift under the same names. It holds the runtime (Runtime.kt).
 */
class XputeModule : Module() {
  private fun bytesOf(a: Uint8Array): ByteArray = ByteArray(a.byteLength).also { a.read(it, 0, a.byteLength) }

  private val main = Handler(Looper.getMainLooper())

  /** This tells the guest's thread each display frame (`Runtime.vsync`). */
  private val frames = object : Choreographer.FrameCallback {
    override fun doFrame(frameTimeNanos: Long) {
      Runtime.vsync((System.nanoTime() - frameTimeNanos) / 1e6)
      Choreographer.getInstance().postFrameCallback(this)
    }
  }

  override fun definition() = ModuleDefinition {
    Name("Xpute")

    /** Fires when frames wait for `take`, when a call waits for `answer`, or
     * when the guest has stopped. It is sent from the guest's thread, and
     * Expo carries the event to the JavaScript thread. */
    Events("onNotify")

    OnCreate {
      Runtime.notify = { sendEvent("onNotify") }
      main.post { Choreographer.getInstance().postFrameCallback(frames) }
    }

    OnDestroy {
      main.post { Choreographer.getInstance().removeFrameCallback(frames) }
      Runtime.notify = null
      Runtime.answer = null
    }

    // The region belongs to the guest for the app's whole life, so nothing
    // is freed when the buffer is.
    Function("mem") { NativeArrayBuffer(Runtime.mem()) }

    Function("wake") { Runtime.wake() }

    Function("input") { Runtime.input() }

    Function("pause") { Runtime.pause() }

    Function("resume") { Runtime.resume() }

    Function("answer") { Runtime.answer() }

    Function("attach") { fs: Boolean -> Runtime.attach(fs) }

    Function("pace") { paceHz: Double, marginShare: Double, batchFrames: Double, settleMs: Double ->
      Runtime.pace(paceHz, marginShare, batchFrames, settleMs)
    }

    Function("post") { tag: Double, cmd: Double, flags: Double, result: Double, pkt: Uint8Array ->
      Runtime.post(tag.toLong().toInt(), cmd.toLong().toInt(), flags.toLong().toInt(), result.toInt(), bytesOf(pkt))
    }

    Function("take") {
      val bytes = Runtime.take()
      NativeArrayBuffer(ByteBuffer.allocateDirect(bytes.size).put(bytes).also { it.flip() })
    }

    Function("fsCompleteOpen") { fd: Double, result: Double, detail: Double, bytes: Uint8Array ->
      Runtime.fsCompleteOpen(fd.toLong().toInt(), result.toInt(), detail.toLong().toInt(), bytesOf(bytes))
    }


    // @xpute/web-host's answer, called on the JavaScript thread, which is the
    // app's thread here.
    Function("callOn") { answer: JavaScriptFunction<Int> -> Runtime.answer = { at -> answer.invoke(at) } }

    Function("log") { line: String -> Log.i("xpute", line); Unit }
  }
}
