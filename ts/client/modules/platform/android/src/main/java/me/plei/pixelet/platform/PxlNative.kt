// @pxl/app/modules/platform/android/src/main/java/me/plei/pixelet/platform/PxlNative.kt

package me.plei.pixelet.platform

import java.nio.ByteBuffer

/**
 * What the app calls in the core (rust/client/core) besides xpute's own calls
 * (xpute.Core, xpute.Gpu): the transport's decoder and the text the core asks for. Kotlin reaches it through
 * the JNI function that the core's src/sys/unpack.rs names, and the iPhone's Swift
 * makes the same call through the C ABI (include/pxl_native.h).
 */
object PxlNative {
  init {
    System.loadLibrary("pxl_core")
  }

  /** Decodes a zstd body into a direct buffer, or returns null when it does
   * not decode. It touches none of the core's state, so it may run on any
   * thread at any time. */
  external fun unpack(packed: ByteArray): ByteBuffer?

  /** Hands the runtime the body of the file `fd`, zstd or not; false where it
   * does not decode, and the runtime has heard nothing. */
  external fun fileBody(fd: Int, body: ByteArray, zstd: Boolean): Boolean

  /** Makes `width` and `raster` answer the core's text, on the core's thread. */
  external fun textOn()

  @JvmStatic fun width(text: ByteArray, weight: Double, px: Double): Double =
    textWidth(String(text, Charsets.UTF_8), weight, px)

  @JvmStatic fun raster(text: ByteArray, weight: Double, px: Double, rgba: Long, x: Double, width: Int, height: Int, out: ByteBuffer) {
    textRaster(String(text, Charsets.UTF_8), weight, px, rgba, x, width, height, out)
  }
}
