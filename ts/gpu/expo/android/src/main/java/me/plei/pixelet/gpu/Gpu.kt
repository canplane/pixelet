// @pxl/gpu/expo/android/src/main/java/me/plei/pixelet/gpu/Gpu.kt

package me.plei.pixelet.gpu

import android.view.Surface
import xpute.BuildConfig

/**
 * This is the GPU's host beside xpute's runtime (@xpute/expo's
 * Runtime.kt), as pxl-gpu-host exports it to JNI (pxl-gpu-host `jni_abi!`,
 * jni.rs). These are the calls the iOS module's Swift makes through the C ABI
 * (pxl_gpu.h). The host opens its device when the guest asks for one. A
 * device the system loses is let go, and the guest is told and asks for a
 * device again. The app is told what the host has of a GPU as it changes
 * (`onState`).
 */
object Gpu {
  init {
    System.loadLibrary(BuildConfig.RUNTIME)
  }

  /** Makes the host tell this class's static `onState` its state
   * (pxl_gpu.h `pxl_gpu_state_on`). */
  external fun init()

  /** What the host has of a GPU now, or null before the guest's first ask is
   * answered (pxl_gpu.h `pxl_gpu_state`). */
  external fun state(): String?

  /** Binds the guest's surface `handle` to `surface`, whose native window the
   * host holds until it is unbound. It binds on the GPU's thread and returns
   * at once: 0, or -EINVAL (-22) where the surface has no native window.
   * Where the system makes no surface of it, or a newer handle has bound the
   * slot since, it is logged and the window let go. While no device is open
   * the window is kept, and the next device binds it. */
  external fun surfaceBind(handle: Int, surface: Surface): Int

  /** Lets `handle`'s window go, and returns once nothing draws into it. */
  external fun surfaceUnbind(handle: Int)

  /** Tells the host of a display frame (pxl_gpu.h `pxl_gpu_vsync`). */
  external fun vsync()

  /** `onState` calls this, and it sends the module's event
   * (PxlGpuModule.kt `onState`). */
  @Volatile var hear: ((state: String) -> Unit)? = null

  /** The GPU's host (pxl-gpu-host jni.rs `state`) calls this with
   * what it has of a GPU, as it changes, from the GPU's thread, which it
   * attached to the VM. */
  @JvmStatic fun onState(state: String) {
    hear?.invoke(state)
  }
}
