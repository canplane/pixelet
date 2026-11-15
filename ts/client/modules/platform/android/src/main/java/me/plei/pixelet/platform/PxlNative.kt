// @pxl/client/modules/platform/android/src/main/java/me/plei/pixelet/platform/PxlNative.kt

package me.plei.pixelet.platform

import java.nio.ByteBuffer

/**
 * What the app calls in its native library (rust/client/runtime) besides
 * xpute's and the GPU's own calls (xpute.Runtime, me.plei.pixelet.gpu.Gpu):
 * the transport's decoder and the text the core asks for. Kotlin reaches it through the JNI functions that the
 * library's src/unpack.rs and src/text.rs name, and the iPhone's Swift makes the
 * same calls through the C ABI (include/pxl_native.h).
 */
object PxlNative {
  init {
    System.loadLibrary("pxl_runtime")
  }

  /** Decodes a zstd body into a direct buffer, or returns null when it does
   * not decode. It touches none of the core's state, so it may run on any
   * thread at any time. */
  external fun unpack(packed: ByteArray): ByteBuffer?

  /** Makes `width` and `raster` answer the core's text, on the core's thread. */
  external fun textOn()

  @JvmStatic fun width(text: ByteArray, weight: Double, px: Double): Double =
    textWidth(String(text, Charsets.UTF_8), weight, px)

  @JvmStatic fun raster(text: ByteArray, weight: Double, px: Double, rgba: Long, x: Double, width: Int, height: Int, out: ByteBuffer) {
    textRaster(String(text, Charsets.UTF_8), weight, px, rgba, x, width, height, out)
  }
}
