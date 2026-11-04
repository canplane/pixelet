// @xpute/native/android/src/main/java/xpute/Guest.kt

package xpute

import expo.modules.kotlin.jni.JavaScriptFunction
import java.nio.ByteBuffer

/**
 * The guest linked into the app, as xpute's native form exports it to JNI
 * (xpute-host `native_guest!`, src/jni.rs): the same calls an iPhone's Swift
 * makes through the C ABI (xpute.h). The guest is the shared library the
 * module's build makes of the app's core crate (build.gradle). It runs on its
 * own thread while it has work; the JavaScript thread kicks it, waits only
 * for a turn it rings, and answers its hostcalls.
 */
object Guest {
  init {
    System.loadLibrary(BuildConfig.CORE)
  }

  /** A turn given at least `quotaMs`, waited for, and when the guest wants
   * the next, in ms; NaN once it has stopped. The first boots the guest. Its
   * hostcalls come back to this thread meanwhile (`hostcall`). */
  external fun run(quotaMs: Double): Double

  /** A turn wanted at once, the next given at least `quotaMs`, none of its
   * own where negative. */
  external fun kick(quotaMs: Double)

  /** The hostcall waiting, if one does, answered on this thread. */
  external fun answer()

  /** The wake the turns since asked, then the longest of them, in ms. */
  external fun heard(): DoubleArray

  /** What answers the guest's hostcalls (@xpute/host), on the JavaScript
   * thread. */
  @Volatile var answer: JavaScriptFunction<Int>? = null

  /** What `notified` calls: the module's event (XputeModule.kt `onNotify`). */
  @Volatile var notify: (() -> Unit)? = null

  /** Called by the guest's host (xpute-host src/jni.rs `hostcall`) on the
   * JavaScript thread, inside `run` or `answer`: the record at `at`,
   * answered before the turn goes on, ENOSYS where nothing answers. */
  @JvmStatic fun hostcall(at: Int): Int = answer?.invoke(at) ?: -38

  /** Called by the guest's host (xpute-host src/jni.rs `notify`) from the
   * guest's thread, attached to the VM there: the guest has posted or asked,
   * a hostcall waits, or the guest has stopped. */
  @JvmStatic fun notified() {
    notify?.invoke()
  }

  /** The memory, a direct buffer over the whole region the guest took. */
  external fun mem(): ByteBuffer
}
