// @xpute/gpu/android/src/main/java/xpute/Gpu.kt

package xpute

import android.view.Surface

/**
 * This is the GPU's runtime beside the core (@xpute/native's Core.kt), as xpute exports it to
 * JNI (xpute-gpu `native_abi!`, src/runtime/jni.rs). These are the same
 * calls that the iOS module's Swift makes through the C ABI (xpute_gpu.h).
 * Each call reaches the runtime on the core's thread while the caller waits,
 * and a completion asks the core for a turn from there. A device the system
 * loses is let go and told (`onLost`), and the next `open` opens a new one.
 */
object Gpu {
  init {
    System.loadLibrary(BuildConfig.CORE)
  }

  /** Whether a GPU device is open, newly or already; after a loss it opens a
   * new one. It keeps its compiled pipelines in `cacheDir`, the directory the
   * app's caches are kept in, or nowhere when it is null. What the device is,
   * the core reads from the runtime. */
  external fun open(cacheDir: String?): Boolean

  /** Binds the core's surface `handle` to `surface`, whose native window the
   * runtime holds until it is unbound. Returns 0, or -2 before `open` is first
   * called, -3 when the system would not make a surface of it, and -4 when a
   * newer handle has bound the same slot since. While no device is open the
   * surface is kept, and the next device binds it. */
  external fun surfaceBind(handle: Int, surface: Surface): Int

  external fun surfaceUnbind(handle: Int)

  /** `onLost` calls this, and it sends the module's event (XputeGpuModule.kt
   * `onLost`). */
  @Volatile var lost: (() -> Unit)? = null

  /** The GPU's runtime (xpute-gpu src/runtime/jni.rs `lost`) calls this once a
   * device is lost and let go, from the thread that found the loss, which it
   * attached to the VM. */
  @JvmStatic fun onLost() {
    lost?.invoke()
  }
}
