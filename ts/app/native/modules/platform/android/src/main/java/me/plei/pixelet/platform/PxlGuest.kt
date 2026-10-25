// @pxl/native/modules/platform/android/src/main/java/me/plei/pixelet/platform/PxlGuest.kt

package me.plei.pixelet.platform

import android.view.Surface
import java.nio.ByteBuffer

/**
 * The kernel as this app calls it: the guest (rust/client-bracket/native),
 * loaded as the shared library the app's build puts beside it, and reached
 * through the functions its src/android.rs names for JNI — the same calls an
 * iPhone's Swift makes through the C ABI. Whatever reaches the kernel runs on
 * the kernel's own thread while the caller waits, so any thread may call, one
 * call at a time.
 */
object PxlGuest {
  init {
    System.loadLibrary("pxl_native")
  }

  /** A turn of `quotaMs`, and when the kernel wants the next, in ms. The
   * first lays the memory out. */
  external fun intr(quotaMs: Double): Double

  /** The memory, a direct buffer over the whole region the guest took. */
  external fun mem(): ByteBuffer

  /** Whether the device opened; what it is, the kernel reads from the host. */
  external fun gpuOpen(): Boolean

  /** What `wake` calls: the module's event (PlatformModule.kt `onWake`). */
  @Volatile var woken: (() -> Unit)? = null

  /** Called by the device's host (src/android.rs `wake`) from whichever
   * thread learned of a completion, attached to the VM there, to ask the turn
   * that reads it. */
  @JvmStatic fun wake() {
    woken?.invoke()
  }

  /** Binds the kernel's surface `handle` to `surface`, which the caller
   * keeps until it has unbound it: 0, -2 when no device is open, -3 when the
   * system would not make a surface of it. */
  external fun surfaceBind(handle: Int, surface: Surface): Int

  /** Lets `handle`'s surface go. */
  external fun surfaceUnbind(handle: Int)

  /** A zstd body decoded into a direct buffer, or null where it does not
   * decode. It never reaches the kernel: any thread, at any time. */
  external fun unpack(packed: ByteArray): ByteBuffer?
}
