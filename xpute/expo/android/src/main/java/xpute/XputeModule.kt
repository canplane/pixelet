// @xpute/expo/android/src/main/java/xpute/XputeModule.kt

package xpute

import expo.modules.kotlin.jni.NativeArrayBuffer
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import expo.modules.kotlin.typedarray.Uint8Array
import java.nio.ByteBuffer

/**
 * This is xpute's native module on Android (src/xpute.ts), the same as
 * ios/XputeModule.swift under the same names: it starts the runtime
 * (Runtime.kt) on the guest's thread, carries the UI's calls to it and back,
 * and tells it of the app's lifecycle itself. A u32 crosses from JavaScript as
 * a Long, which holds it whole, and reaches JNI as the Int of its bits.
 */
class XputeModule : Module() {
  private fun bytesOf(a: Uint8Array): ByteArray = ByteArray(a.byteLength).also { a.read(it, 0, a.byteLength) }

  /** The UI's handle on the runtime, which its start answers and each post,
   * answer and take names, so an old JavaScript runtime's reaches nothing
   * once a new one has started (xpute.h `xpute_start`); 0, no UI's, until
   * then. */
  private var ui = 0

  /** The guest's stop as `onStopped` carries it. A JavaScript number holds
   * no u64 whole, so each crosses as its decimal digits. */
  private fun report(s: Stop) = mapOf(
    "errnum" to s.errnum,
    "file" to s.file,
    "line" to s.line.toUInt().toLong(),
    "n1" to s.n1.toULong().toString(),
    "n2" to s.n2.toULong().toString(),
  )

  /** These send the module's events from the runtime's threads, and Expo
   * carries each to the JavaScript thread. */
  private val notify = { sendEvent("onNotify") }
  private val stopped = { s: Stop -> sendEvent("onStopped", report(s)) }
  private val trapped = { err: String -> sendEvent("onTrap", mapOf("err" to err)) }

  override fun definition() = ModuleDefinition {
    Name("Xpute")

    /** `onNotify` fires when a batch waits for `take`, `onStopped` when the
     * guest stopped, with its report, and `onTrap` when it trapped, with what
     * stopped it. */
    Events("onNotify", "onStopped", "onTrap")

    // The runtime tells the newest module made, and a module clears the
    // runtime's callbacks only while they are its own, under the lock of
    // `Runtime`, as on iOS, where a reload may make the new module before the
    // old one is destroyed.
    OnCreate {
      synchronized(Runtime) {
        Runtime.notify = notify
        Runtime.stopped = stopped
        Runtime.trapped = trapped
      }
      Runtime.init()
    }

    OnDestroy {
      synchronized(Runtime) {
        if (Runtime.notify === notify) {
          Runtime.notify = null
          Runtime.stopped = null
          Runtime.trapped = null
        }
      }
    }

    OnActivityEntersBackground { Runtime.life(false) }

    OnActivityEntersForeground { Runtime.life(true) }

    Function("start") { named: LongArray, prefixes: Uint8Array ->
      ui = Runtime.start(
        IntArray(named.size) { named[it].toInt() },
        bytesOf(prefixes),
        appContext.cacheDirectory.path,
      )
    }

    Function("post") { tag: Long, h: Long, m: Long, pkt: Uint8Array? ->
      Runtime.post(ui, tag.toInt(), h.toInt(), m.toInt(), pkt?.let(::bytesOf))
    }

    Function("answer") { tag: Long, result: Int, data: Uint8Array? ->
      Runtime.answer(ui, tag.toInt(), result, data?.let(::bytesOf))
    }

    Function("take") {
      val bytes = Runtime.take(ui)
      NativeArrayBuffer(ByteBuffer.allocateDirect(bytes.size).put(bytes).also { it.flip() })
    }

    // A span the guest lent, its address in two words, `lo` and `hi`: the
    // buffer is the UI's until the lend's answer, and nothing is freed when
    // the buffer is.
    Function("bytes") { lo: Long, hi: Long, len: Int ->
      NativeArrayBuffer(Runtime.bytes((hi shl 32) or lo, len))
    }

    /** The guest's last stop, as `onStopped` carries it, or null before the
     * guest stopped (`Runtime.stop()`). */
    Function("stop") { Runtime.stop()?.let(::report) }

    /** What trapped the guest, or null before it trapped (`Runtime.trap()`). */
    Function("trap") { Runtime.trap() }

    Function("log") { level: Int, context: String, message: String ->
      Runtime.log(level, context, message)
    }
  }
}
