// @xpute/native-runtime/android/src/main/java/xpute/Runtime.kt

package xpute

import java.nio.ByteBuffer

/**
 * This is xpute's runtime on Android as JNI reaches it: the calls the iOS
 * module's Swift makes through the C ABI (xpute.h), exported by the app's
 * native library, which the module's build names (build.gradle). The runtime
 * runs on the core's thread; each call posts its task there and returns. It
 * tells of a batch, a stop and a trap through the static callbacks below,
 * which it calls from its threads, attached to the VM as daemons.
 * It takes nothing of React Native's; the Expo module (XputeModule.kt) is one
 * app's way to reach it.
 */
object Runtime {
  init {
    System.loadLibrary(BuildConfig.RUNTIME)
  }

  /** Makes the runtime reach this class's static callbacks. Called before
   * `start`. */
  external fun init()

  /** Starts the runtime on the core's thread, once (xpute.h `xpute_start`). */
  external fun start(named: IntArray, prefixes: ByteArray, filesDir: String, cacheDir: String)

  /** Sends the UI's method `m` of the guest's object `h` (xpute.h
   * `xpute_post`); null `pkt` for none. */
  external fun post(tag: Int, h: Int, m: Int, pkt: ByteArray?)

  /** Answers the request `tag` the runtime sent the UI (xpute.h
   * `xpute_answer`); null `data` for none. */
  external fun answer(tag: Int, result: Int, data: ByteArray?)

  /** Hands over the batch the UI has not taken (xpute.h `xpute_take`). */
  external fun take(): ByteArray

  /** The core's memory, a direct buffer over its whole region. */
  external fun mem(): ByteBuffer

  /** Tells the runtime that the app was hidden or shown. */
  external fun life(shown: Boolean)

  /** These send the module's events (XputeModule.kt). */
  @Volatile var notify: (() -> Unit)? = null
  @Volatile var stopped: ((errnum: Int, file: String, line: Int, n1: Long, n2: Long) -> Unit)? = null
  @Volatile var trap: (() -> Unit)? = null

  /** A batch waits for `take`. */
  @JvmStatic fun onNotify() {
    notify?.invoke()
  }

  /** The guest stopped, with its report; `line`, `n1` and `n2` are unsigned. */
  @JvmStatic fun onStopped(errnum: Int, file: String, line: Int, n1: Long, n2: Long) {
    stopped?.invoke(errnum, file, line, n1, n2)
  }

  /** The guest trapped. */
  @JvmStatic fun onTrap() {
    trap?.invoke()
  }
}
