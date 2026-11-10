// @xpute/gpu/android/src/main/java/xpute/Gpu.kt

package xpute

import android.view.Surface

/**
 * This is the GPU's runtime beside the core (@xpute/native's Core.kt), as xpute exports it to
 * JNI (xpute-gpu `native_abi!`, src/runtime/jni.rs). These are the same
 * calls that the iOS module's Swift makes through the C ABI (xpute_gpu.h).
 * Each call reaches the runtime on the core's thread while the caller waits,
 * and a completion asks the core for a turn from there.
 */
object Gpu {
  init {
    System.loadLibrary(BuildConfig.CORE)
  }

  /** Whether the GPU device opened. What the device is, the core reads from
   * the runtime. */
  external fun open(): Boolean

  /** Binds the core's surface `handle` to `surface`, which the caller must
   * keep until it has unbound it. Returns 0, or -2 when no device is open,
   * -3 when the system would not make a surface of it, and -4 when a newer
   * handle has bound the same slot since. */
  external fun surfaceBind(handle: Int, surface: Surface): Int

  external fun surfaceUnbind(handle: Int)
}
