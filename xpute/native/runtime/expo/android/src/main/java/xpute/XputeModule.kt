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
 * (Runtime.kt) on the guest's thread, carries the UI's link to it and back,
 * and tells it of the app's lifecycle itself. A u32 crosses from JavaScript as
 * a Long, which holds it whole, and reaches JNI as the Int of its bits.
 */
class XputeModule : Module() {
  private fun bytesOf(a: Uint8Array): ByteArray = ByteArray(a.byteLength).also { a.read(it, 0, a.byteLength) }

  /** The UI's generation, which its start answers and each post, answer and
   * take carries, so an old JavaScript runtime's reaches nothing once a new
   * one has started (xpute.h `xpute_start`); 0, no UI's, until then. */
  private var generation = 0

  /** These send the module's events from the runtime's threads, a stop and
   * a trap also from a start after them, and Expo carries each to the
   * JavaScript thread. */
  private val notify = { sendEvent("onNotify") }
  // A JavaScript number holds no u64 whole, so each crosses as its decimal
  // digits.
  private val stopped = { errnum: Int, file: String, line: Int, n1: Long, n2: Long ->
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
  private val trap = { sendEvent("onTrap") }

  override fun definition() = ModuleDefinition {
    Name("Xpute")

    /** `onNotify` fires when a batch waits for `take`, `onStopped` when the
     * guest stopped, with its report, and `onTrap` when it trapped. */
    Events("onNotify", "onStopped", "onTrap")

    // The runtime tells the newest module made, and a module clears the
    // runtime's callbacks only while they are its own, under the lock of
    // `Runtime`, as on iOS, where a reload may make the new module before the
    // old one is destroyed.
    OnCreate {
      synchronized(Runtime) {
        Runtime.notify = notify
        Runtime.stopped = stopped
        Runtime.trap = trap
      }
      Runtime.init()
    }

    OnDestroy {
      synchronized(Runtime) {
        if (Runtime.notify === notify) {
          Runtime.notify = null
          Runtime.stopped = null
          Runtime.trap = null
        }
      }
    }

    OnActivityEntersBackground { Runtime.life(false) }

    OnActivityEntersForeground { Runtime.life(true) }

    Function("start") { named: LongArray, prefixes: Uint8Array ->
      generation = Runtime.start(
        IntArray(named.size) { named[it].toInt() },
        bytesOf(prefixes),
        appContext.cacheDirectory.path,
      )
    }

    Function("post") { tag: Long, h: Long, m: Long, pkt: Uint8Array? ->
      Runtime.post(generation, tag.toInt(), h.toInt(), m.toInt(), pkt?.let(::bytesOf))
    }

    Function("answer") { tag: Long, result: Int, data: Uint8Array? ->
      Runtime.answer(generation, tag.toInt(), result, data?.let(::bytesOf))
    }

    Function("take") {
      val bytes = Runtime.take(generation)
      NativeArrayBuffer(ByteBuffer.allocateDirect(bytes.size).put(bytes).also { it.flip() })
    }

    // The region is the guest's for the app's whole life, so nothing is freed
    // when the buffer is.
    Function("mem") { NativeArrayBuffer(Runtime.mem()) }

    Function("log") { line: String -> Log.i("xpute", line); Unit }
  }
}
