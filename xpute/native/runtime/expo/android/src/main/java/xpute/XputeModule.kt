// @xpute/native-runtime/android/src/main/java/xpute/XputeModule.kt

package xpute

import android.util.Log
import expo.modules.kotlin.jni.NativeArrayBuffer
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import expo.modules.kotlin.typedarray.Uint8Array
import java.nio.ByteBuffer

/**
 * This is xpute's native module on Android (src/mod.ts), the same as
 * ios/XputeModule.swift under the same names: it starts the runtime
 * (Runtime.kt) on the core's thread, carries the UI's link to it and back,
 * and tells it of the app's lifecycle itself. A u32 crosses from JavaScript as
 * a Long, which holds it whole, and reaches JNI as the Int of its bits.
 */
class XputeModule : Module() {
  private fun bytesOf(a: Uint8Array): ByteArray = ByteArray(a.byteLength).also { a.read(it, 0, a.byteLength) }

  override fun definition() = ModuleDefinition {
    Name("Xpute")

    /** `onNotify` fires when a batch waits for `take`, `onStopped` when the
     * guest stopped, with its report, and `onTrap` when it trapped. Each
     * is sent from the runtime's threads, and Expo carries it to the
     * JavaScript thread. */
    Events("onNotify", "onStopped", "onTrap")

    OnCreate {
      Runtime.notify = { sendEvent("onNotify") }
      // A JavaScript number holds no u64 whole, so each crosses as its
      // decimal digits.
      Runtime.stopped = { errnum, file, line, n1, n2 ->
        sendEvent(
          "onStopped",
          mapOf(
            "errnum" to errnum,
            "file" to file,
            "line" to line.toUInt().toLong(),
            "n1" to n1.toULong().toString(),
            "n2" to n2.toULong().toString(),
          ),
        )
      }
      Runtime.trap = { sendEvent("onTrap") }
      Runtime.init()
    }

    OnDestroy {
      Runtime.notify = null
      Runtime.stopped = null
      Runtime.trap = null
    }

    OnActivityEntersBackground { Runtime.life(false) }

    OnActivityEntersForeground { Runtime.life(true) }

    Function("start") { named: LongArray, prefixes: Uint8Array ->
      Runtime.start(
        IntArray(named.size) { named[it].toInt() },
        bytesOf(prefixes),
        appContext.persistentFilesDirectory.path,
        appContext.cacheDirectory.path,
      )
    }

    Function("post") { tag: Long, h: Long, m: Long, pkt: Uint8Array? ->
      Runtime.post(tag.toInt(), h.toInt(), m.toInt(), pkt?.let(::bytesOf))
    }

    Function("answer") { tag: Long, result: Int, data: Uint8Array? ->
      Runtime.answer(tag.toInt(), result, data?.let(::bytesOf))
    }

    Function("take") {
      val bytes = Runtime.take()
      NativeArrayBuffer(ByteBuffer.allocateDirect(bytes.size).put(bytes).also { it.flip() })
    }

    // The region is the core's for the app's whole life, so nothing is freed
    // when the buffer is.
    Function("mem") { NativeArrayBuffer(Runtime.mem()) }

    Function("log") { line: String -> Log.i("xpute", line); Unit }
  }
}
