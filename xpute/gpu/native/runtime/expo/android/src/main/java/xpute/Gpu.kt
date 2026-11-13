// @xpute/gpu-native-runtime/android/src/main/java/xpute/Gpu.kt

package xpute

import android.view.Surface

/**
 * This is the GPU's host beside xpute's (@xpute/native-runtime's Runtime.kt), as
 * xpute exports it to JNI (xpute-gpu-native-host `jni_abi!`, jni.rs). These
 * are the same calls that the iOS module's Swift makes through the C ABI
 * (xpute_gpu.h). Each call reaches the host on the guest's thread while the
 * caller waits, and a completion asks the guest for an entry from there. A device the system
 * loses is let go and told (`onLost`), and the next `open` opens a new one.
 */
object Gpu {
  init {
    System.loadLibrary(BuildConfig.GUEST)
  }

  /** Whether a GPU device is open, newly or already; after a loss it opens a
   * new one. It keeps its compiled pipelines in `cacheDir`, the directory the
   * app's caches are kept in, or nowhere when it is null. What the device is,
   * the guest reads from the host. */
  external fun open(cacheDir: String?): Boolean

  /** Binds the guest's surface `handle` to `surface`, whose native window the
   * host holds until it is unbound. Returns 0, or -ENODEV (-19) before
   * `open` is first called, -EINVAL (-22) when the system would not make a
   * surface of it, and -EBUSY (-16) when a newer handle has bound the same slot
   * since. While no device is open the
   * surface is kept, and the next device binds it. */
  external fun surfaceBind(handle: Int, surface: Surface): Int

  external fun surfaceUnbind(handle: Int)

  /** `onLost` calls this, and it sends the module's event (XputeGpuModule.kt
   * `onLost`). */
  @Volatile var lost: (() -> Unit)? = null

  /** The GPU's host (xpute-gpu-native-host jni.rs `lost`) calls this once a
   * device is lost and let go, from the thread that found the loss, which it
   * attached to the VM. */
  @JvmStatic fun onLost() {
    lost?.invoke()
  }
}
