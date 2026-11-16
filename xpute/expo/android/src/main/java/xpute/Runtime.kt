// @xpute/expo/android/src/main/java/xpute/Runtime.kt

package xpute

import java.nio.ByteBuffer

/**
 * This is xpute's runtime on Android as JNI reaches it: the calls the iOS
 * module's Swift makes through the C ABI (xpute.h), exported by the app's
 * native library, which the module's build names (build.gradle). The runtime
 * runs on the guest's thread; each call posts its task there and returns. It
 * tells of a batch, a stop and a trap through the static callbacks below,
 * which it calls from its threads, attached to the VM as daemons, and keeps
 * the stop and the trap for a UI that starts after them (`stop`, `trap`).
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

  /** Starts the runtime on the guest's thread, once a process; each call
   * attaches a new UI to it, as a reload's, and answers its handle (xpute.h
   * `xpute_start`). */
  external fun start(named: IntArray, prefixes: ByteArray, cacheDir: String): Int

  /** Sends the method `m` of the guest's object `h` of the UI of the handle
   * `ui` (xpute.h `xpute_post`); null `pkt` for none. */
  external fun post(ui: Int, tag: Int, h: Int, m: Int, pkt: ByteArray?)

  /** Answers the request `tag` the runtime sent the UI of the handle `ui`
   * (xpute.h `xpute_answer`); null `data` for none. */
  external fun answer(ui: Int, tag: Int, result: Int, data: ByteArray?)

  /** Hands over the batch the UI of the handle `ui` has not taken (xpute.h
   * `xpute_take`). */
  external fun take(ui: Int): ByteArray

  /** The `len` bytes the guest lent at `at`, a direct buffer over them. */
  external fun bytes(at: Long, len: Int): ByteBuffer

  /** Tells the runtime that the app was hidden or shown. */
  external fun life(shown: Boolean)

  /** The guest's last stop, or null before the guest stopped (xpute.h
   * `xpute_stop`). */
  external fun stop(): Stop?

  /** What trapped the guest, or null before it trapped (xpute.h
   * `xpute_trap`). */
  external fun trap(): String?

  /** Writes a log record of the UI's by the runtime's writer (xpute.h
   * `xpute_log`). */
  external fun log(level: Int, context: String, message: String)

  /** These send the module's events (XputeModule.kt). */
  @Volatile var notify: (() -> Unit)? = null
  @Volatile var stopped: ((Stop) -> Unit)? = null
  @Volatile var trapped: ((err: String) -> Unit)? = null

  /** A batch waits for `take`. */
  @JvmStatic fun onNotify() {
    notify?.invoke()
  }

  /** The guest stopped, with its report. */
  @JvmStatic fun onStopped(stop: Stop) {
    stopped?.invoke(stop)
  }

  /** The guest trapped, with what stopped it. */
  @JvmStatic fun onTrap(err: String) {
    trapped?.invoke(err)
  }
}

/** The guest's stop, its report: its errno, its file, its line and its two
 * numbers, `line`, `n1` and `n2` unsigned (xpute-runtime-native listener.rs
 * `Stop`). */
class Stop(val errnum: Int, val file: String, val line: Int, val n1: Long, val n2: Long)
