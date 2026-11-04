// @xpute/native/android/src/main/java/xpute/Guest.kt

package xpute

import expo.modules.kotlin.jni.JavaScriptFunction
import java.nio.ByteBuffer

/**
 * The guest linked into the app, as xpute's native form exports it to JNI
 * (xpute-host `native_guest!`, src/jni.rs): the same calls an iPhone's Swift
 * makes through the C ABI (xpute.h). The guest is the shared library the
 * module's build makes of the app's core crate (build.gradle). Whatever
 * reaches the guest runs on the guest's own thread while the caller waits, so
 * any thread may call, one call at a time.
 */
object Guest {
  init {
    System.loadLibrary(BuildConfig.CORE)
  }

  /** A turn of `quotaMs`, and when the guest wants the next, in ms. The
   * first lays the memory out. Its hostcalls come back to this thread
   * (`hostcall`). */
  external fun run(quotaMs: Double): Double

  /** What answers the guest's hostcalls (@xpute/host), on the JavaScript
   * thread. */
  @Volatile var answer: JavaScriptFunction<Int>? = null

  /** Called by the guest's host (xpute-host src/jni.rs `hostcall`) on the
   * thread waiting in `run`, the JavaScript thread: the record at `at`,
   * answered before the turn goes on, ENOSYS where nothing answers. */
  @JvmStatic fun hostcall(at: Int): Int = answer?.invoke(at) ?: -38

  /** The memory, a direct buffer over the whole region the guest took. */
  external fun mem(): ByteBuffer
}
