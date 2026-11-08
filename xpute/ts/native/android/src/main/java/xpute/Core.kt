// @xpute/native/android/src/main/java/xpute/Core.kt

package xpute

import expo.modules.kotlin.jni.JavaScriptFunction
import java.nio.ByteBuffer

/**
 * The core linked into the app, as xpute's native form exports it to JNI
 * (xpute-runtime `native_core!`, src/jni.rs). These are the same calls that
 * the iOS module's Swift makes through the C ABI (xpute.h). The core is the
 * shared library that the module's build makes of the app's core crate
 * (build.gradle). It runs on its own thread while it has work; the
 * JavaScript thread kicks it, waits on it only for a turn it rings, and
 * answers its calls.
 */
object Core {
  init {
    System.loadLibrary(BuildConfig.CORE)
  }

  /** Rings a turn that ends no sooner than `deadlineMs` after its rise, and
   * waits for it. Returns when the core wants the next turn, in ms, or NaN
   * once the core has stopped. The first call boots the core. The turn's
   * calls come back to this thread meanwhile (`call`). */
  external fun run(deadlineMs: Double): Double

  /** Asks for a turn at once. The next turn ends no sooner than `deadlineMs`
   * after its rise; a negative `deadlineMs` sets no deadline. */
  external fun kick(deadlineMs: Double)

  /** From now on only a turn `run` rings may begin, and a kick waits for
   * `resume`. */
  external fun pause()

  /** Turns begin again as kicks ask. */
  external fun resume()

  /** Answers on this thread the call that waits, if one does. */
  external fun answer()

  /** What the turns since the last call asked: the wake, then how long the
   * longest of them took, in ms. */
  external fun turns(): DoubleArray

  /** The JavaScript function that answers the core's calls (@xpute/runtime),
   * on the JavaScript thread. */
  @Volatile var answer: JavaScriptFunction<Int>? = null

  /** What `onNotify` calls: it sends the module's event (XputeModule.kt
   * `onNotify`). */
  @Volatile var notify: (() -> Unit)? = null

  /** The core's runtime (xpute-runtime src/jni.rs `call`) calls this on the
   * JavaScript thread, inside `run` or `answer`, for the call laid at `at`.
   * The call is answered before the turn goes on, with ENOSYS (-38) when
   * nothing answers. */
  @JvmStatic fun call(at: Int): Int = answer?.invoke(at) ?: -38

  /** The core's runtime (xpute-runtime src/jni.rs `notify`) calls this from
   * the core's thread, which it attached to the VM, when the core has posted
   * or asked for a turn, when a call waits, or when the core has stopped. */
  @JvmStatic fun onNotify() {
    notify?.invoke()
  }

  /** The memory, a direct buffer over the whole region the core took. */
  external fun mem(): ByteBuffer

  /** Loads the word at byte `at` of the memory with acquire. The word
   * crosses as a Double, since Kotlin's Int cannot hold every u32. */
  external fun load(at: Double): Double

  /** Stores `word` at byte `at` of the memory with release. */
  external fun store(at: Double, word: Double)
}
