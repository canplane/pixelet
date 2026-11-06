// @xpute/native/android/src/main/java/xpute/Gpu.kt

package xpute

import android.view.Surface

/**
 * The GPU's runtime beside the core (Core.kt), as xpute exports it to JNI
 * (xpute-gpu `native_abi!`, src/runtime/native/jni.rs): the same calls an
 * iPhone's Swift makes through the C ABI (xpute_gpu.h). Each reaches the runtime
 * on the core's thread while the caller waits; a completion kicks the core
 * there.
 */
object Gpu {
  init {
    System.loadLibrary(BuildConfig.CORE)
  }

  /** Whether the device opened; what it is, the core reads from the runtime. */
  external fun open(): Boolean

  /** Binds the core's surface `handle` to `surface`, which the caller
   * keeps until it has unbound it: 0, -2 when no device is open, -3 when the
   * system would not make a surface of it, -4 when a newer handle has bound
   * its slot since. */
  external fun surfaceBind(handle: Int, surface: Surface): Int

  /** Lets `handle`'s surface go. */
  external fun surfaceUnbind(handle: Int)
}
