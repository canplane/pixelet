// @xpute/gpu-native-runtime/android/src/main/java/xpute/Gpu.kt

package xpute

import android.view.Surface

/**
 * This is the GPU's host beside xpute's runtime (@xpute/native-runtime's
 * Runtime.kt), as xpute exports it to JNI (xpute-gpu-native-host `jni_abi!`,
 * jni.rs). These are the calls the iOS module's Swift makes through the C ABI
 * (xpute_gpu.h). The host opens its device when the guest asks for one. A
 * device the system loses is let go, and the guest is told and asks for a
 * device again. The app is told what the host has of a GPU as it changes
 * (`onState`).
 */
object Gpu {
  init {
    System.loadLibrary(BuildConfig.RUNTIME)
  }

  /** Makes the host tell this class's static `onState` its state
   * (xpute_gpu.h `xpute_gpu_state_on`). */
  external fun init()

  /** Binds the guest's surface `handle` to `surface`, whose native window the
   * host holds until it is unbound. It binds on the GPU's thread and returns
   * at once: 0, or -EINVAL (-22) where the surface has no native window.
   * Where the system makes no surface of it, or a newer handle has bound the
   * slot since, it is logged and the window let go. While no device is open
   * the window is kept, and the next device binds it. */
  external fun surfaceBind(handle: Int, surface: Surface): Int

  /** Lets `handle`'s window go, and returns once nothing draws into it. */
  external fun surfaceUnbind(handle: Int)

  /** Tells the host of a display frame (xpute_gpu.h `xpute_gpu_vsync`). */
  external fun vsync()

  /** `onState` calls this, and it sends the module's event
   * (XputeGpuModule.kt `onState`). */
  @Volatile var state: ((state: String) -> Unit)? = null

  /** The GPU's host (xpute-gpu-native-host jni.rs `state`) calls this with
   * what it has of a GPU, as it changes, from the GPU's thread, which it
   * attached to the VM. */
  @JvmStatic fun onState(state: String) {
    this.state?.invoke(state)
  }
}
