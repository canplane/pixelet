// @xpute/native/android/src/main/java/xpute/Core.kt

package xpute

import expo.modules.kotlin.jni.JavaScriptFunction
import java.nio.ByteBuffer

/**
 * The core linked into the app, as xpute's native form exports it to JNI
 * (xpute-runtime `native_core!`, src/jni.rs): the same calls an iPhone's Swift
 * makes through the C ABI (xpute.h). The core is the shared library the
 * module's build makes of the app's core crate (build.gradle). It runs on its
 * own thread while it has work; the JavaScript thread kicks it, waits only
 * for a turn it rings, and answers its calls.
 */
object Core {
  init {
    System.loadLibrary(BuildConfig.CORE)
  }

  /** A turn ending no sooner than `deadlineMs` from its rise, waited for, and when the core wants
   * the next, in ms; NaN once it has stopped. The first boots the core. Its
   * calls come back to this thread meanwhile (`call`). */
  external fun run(deadlineMs: Double): Double

  /** A turn wanted at once, the next ending no sooner than `deadlineMs` from its rise, none of its
   * own where negative. */
  external fun kick(deadlineMs: Double)

  /** No turn begins but one `run` rings; a kick waits for `resume`. */
  external fun pause()

  /** Turns begin again as kicks ask. */
  external fun resume()

  /** The call waiting, if one does, answered on this thread. */
  external fun answer()

  /** The wake the turns since asked, then the longest of them, in ms. */
  external fun turns(): DoubleArray

  /** What answers the core's calls (@xpute/runtime), on the JavaScript
   * thread. */
  @Volatile var answer: JavaScriptFunction<Int>? = null

  /** What `onNotify` calls: the module's event (XputeModule.kt `onNotify`). */
  @Volatile var notify: (() -> Unit)? = null

  /** Called by the core's runtime (xpute-runtime src/jni.rs `call`) on the
   * JavaScript thread, inside `run` or `answer`: the record at `at`,
   * answered before the turn goes on, ENOSYS where nothing answers. */
  @JvmStatic fun call(at: Int): Int = answer?.invoke(at) ?: -38

  /** Called by the core's runtime (xpute-runtime src/jni.rs `notify`) from the
   * core's thread, attached to the VM there: the core has posted or asked,
   * a call waits, or the core has stopped. */
  @JvmStatic fun onNotify() {
    notify?.invoke()
  }

  /** The memory, a direct buffer over the whole region the core took. */
  external fun mem(): ByteBuffer

  /** The word at byte `at` of the memory, loaded with acquire, a Double since
   * Kotlin's Int stops short of a u32. */
  external fun load(at: Double): Double

  /** `word` stored at byte `at` of the memory with release. */
  external fun store(at: Double, word: Double)
}
