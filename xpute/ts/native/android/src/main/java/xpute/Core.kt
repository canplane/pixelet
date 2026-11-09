// @xpute/native/android/src/main/java/xpute/Core.kt

package xpute

import java.nio.ByteBuffer

/**
 * This is the core linked into the app, as xpute's native form exports it to
 * JNI (xpute-runtime `native_core!`, src/jni.rs). These are the same calls that
 * the iOS module's Swift makes through the C ABI (xpute.h). The core is the
 * shared library that the module's build makes of the app's core crate
 * (build.gradle). The core runs on its own thread, which begins its turns at
 * the display's frames; the app's thread asks for turns, waits on it only for a
 * turn it rings, and answers its calls.
 * It takes nothing of React Native's; the Expo module (XputeModule.kt) is one
 * host that uses it.
 */
object Core {
  init {
    System.loadLibrary(BuildConfig.CORE)
  }

  /** Asks for a turn at the next display frame. */
  external fun wake()

  /** Says that an input came now. */
  external fun input()

  /** Tells the core's thread of a display frame, `ageMs` after its time. */
  external fun vsync(ageMs: Double)

  /** From now on only a turn `run` rings may begin. */
  external fun pause()

  /** Turns begin again at the display's frames. */
  external fun resume()

  /** Answers on this thread the call that waits, if one does. */
  external fun answer()

  /** Starts the runtime's own thread over the rings at the given offsets. */
  external fun attach(sqAt: Int, cqAt: Int, fs: Boolean)

  /** A touch on a surface: phase 0 down, 1 move, 2 up, 3 out, 4 cancel. */
  external fun touch(phase: Int, id: Int, x: Double, y: Double)

  /** The program's numbers for frame pacing, given before the turns run. */
  external fun pace(paceHz: Double, marginShare: Double, batchFrames: Double, settleMs: Double)

  /** Queues a frame for the submission ring. */
  external fun post(tag: Int, cmd: Int, flags: Int, result: Int, pkt: ByteArray)

  /** Hands over what the UI has not heard (xpute.h `xpute_take`). */
  external fun take(): ByteArray

  /** Completes the open of `fd`. */
  external fun fsOpened(fd: Int, result: Int, detail: Int, bytes: ByteArray)

  /** Completes the close of `fd`. */
  external fun fsClosed(fd: Int, result: Int)

  /** Calls the core and waits for its reply (xpute.h `xpute_call`). */
  external fun call(cmd: Int, pkt: ByteArray): Int

  /** The function that answers the core's call laid at the byte it is given,
   * on the app's thread. */
  @Volatile var answer: ((Int) -> Int)? = null

  /** `onNotify` calls this, and it sends the module's event (XputeModule.kt
   * `onNotify`). */
  @Volatile var notify: (() -> Unit)? = null

  /** The core's runtime (xpute-runtime src/jni.rs `call`) calls this on the
   * app's thread, inside `run` or `answer`, for the call laid at `at`.
   * The call is answered before the turn goes on, with ENOSYS (-38) when
   * nothing answers. */
  @JvmStatic fun call(at: Int): Int = answer?.invoke(at) ?: -38

  /** The core's runtime (xpute-runtime src/jni.rs `notify`) calls this from
   * the core's thread, which it attached to the VM, when a turn has ended,
   * when a call waits, or when the core has stopped. */
  @JvmStatic fun onNotify() {
    notify?.invoke()
  }

  /** The memory, a direct buffer over the whole region the core took. */
  external fun mem(): ByteBuffer

  /** Loads the word at byte `at` of the memory with acquire. The word
   * crosses as a Double, since Kotlin's Int cannot hold every u32. */
  external fun load(at: Double): Double

  /** Stores `word` at byte `at` of the memory with release. */
  external fun store(at: Double, word: Double)
}
