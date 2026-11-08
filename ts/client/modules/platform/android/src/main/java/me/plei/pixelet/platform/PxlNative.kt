// @pxl/app/modules/platform/android/src/main/java/me/plei/pixelet/platform/PxlNative.kt

package me.plei.pixelet.platform

import java.nio.ByteBuffer

/**
 * What the app calls in the core (rust/client/core) besides xpute's own calls
 * (xpute.Core, xpute.Gpu): the transport's decoder. Kotlin reaches it through
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
}
