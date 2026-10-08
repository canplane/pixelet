// @pxl/native/modules/platform/android/src/main/java/me/plei/pixelet/platform/PxlGuest.kt

package me.plei.pixelet.platform

import android.view.Surface
import java.nio.ByteBuffer

/**
 * The kernel as this app calls it: the guest (crates/client-bracket/native),
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
  external fun interrupt(quotaMs: Double): Double

  /** The memory, a direct buffer over the whole region the guest took. */
  external fun memory(): ByteBuffer

  /** The device's uniform offset alignment and whether it times its passes,
   * or null where the system has no GPU. */
  external fun gpuOpen(): IntArray?

  /** Decodes a PNG or a WebP and holds it for the renderer as the kernel's
   * `handle`: 1, or 0 when it does not decode. */
  external fun imageHold(handle: Int, file: ByteArray): Int

  /** Binds the kernel's surface `handle` to `surface`, which the caller
   * keeps until it has unbound it: 0, -2 when no device is open, -3 when the
   * system would not make a surface of it. */
  external fun surfaceBind(handle: Int, surface: Surface): Int

  /** Lets `handle`'s surface go. */
  external fun surfaceUnbind(handle: Int)
}
