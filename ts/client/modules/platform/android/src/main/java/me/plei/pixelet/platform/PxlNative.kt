// @pxl/app/modules/platform/android/src/main/java/me/plei/pixelet/platform/PxlNative.kt

package me.plei.pixelet.platform

import java.nio.ByteBuffer

/**
 * What the app calls in the core (rust/client/core) besides xpute's own
 * (xpute.Core, xpute.Gpu): the transport's decoder, reached through the
 * function its src/unpack.rs names for JNI, the same call an iPhone's Swift
 * makes through the C ABI (include/pxl_native.h).
 */
object PxlNative {
  init {
    System.loadLibrary("pxl_core")
  }

  /** A zstd body decoded into a direct buffer, or null where it does not
   * decode. It never reaches the core: any thread, at any time. */
  external fun unpack(packed: ByteArray): ByteBuffer?
}
