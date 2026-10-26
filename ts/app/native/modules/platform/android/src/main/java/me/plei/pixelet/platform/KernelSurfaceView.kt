// @pxl/native/modules/platform/android/src/main/java/me/plei/pixelet/platform/KernelSurfaceView.kt

package me.plei.pixelet.platform

import android.content.Context
import android.view.SurfaceHolder
import android.view.SurfaceView
import expo.modules.kotlin.AppContext
import expo.modules.kotlin.viewevent.EventDispatcher
import expo.modules.kotlin.views.ExpoView

/**
 * A view the kernel draws into (rust/client-bracket/native), SurfaceView.swift's
 * on Android: a SurfaceView, whose surface is a window of its own the system
 * composes under the app's views, so the kernel's frames and the shell's
 * chrome are drawn apart. The view binds the kernel's surface to it itself:
 * the surface the shell names (`surface`), while the system's surface exists,
 * and never past the moment the system takes it back — `surfaceDestroyed`
 * does not return until the kernel has let it go. It says when it is bound
 * (`onAttach`) and when it is not (`onDetach`). The kernel sizes the surface
 * and draws every frame inside a turn, so nothing here draws or runs a frame.
 */
class KernelSurfaceView(context: Context, appContext: AppContext) : ExpoView(context, appContext), SurfaceHolder.Callback {
  private val onAttach by EventDispatcher()
  private val onDetach by EventDispatcher()

  private val view = SurfaceView(context).also {
    it.holder.addCallback(this)
    addView(it)
  }

  /** The kernel's surface the shell wants shown here; 0 for none. */
  var surface: Int = 0
    set(value) {
      field = value
      rebind()
    }

  /** Whether the system's surface exists to be drawn into. */
  private var ready = false

  /** The surface bound to the view's; 0 for none. */
  private var bound = 0

  // React Native lays this view out and none of its own children, so the
  // SurfaceView is given the whole of it here.
  override fun onLayout(changed: Boolean, l: Int, t: Int, r: Int, b: Int) {
    view.layout(0, 0, r - l, b - t)
  }

  override fun surfaceCreated(holder: SurfaceHolder) {
    ready = true
    rebind()
  }

  override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {}

  override fun surfaceDestroyed(holder: SurfaceHolder) {
    ready = false
    rebind()
  }

  /** Binds the kernel's surface to what is wanted, letting go of what is not. */
  private fun rebind() {
    val wanted = if (ready) surface else 0
    if (bound == wanted) return
    if (bound != 0) {
      PxlNative.surfaceUnbind(bound)
      bound = 0
      onDetach(mapOf<String, Any>())
    }
    if (wanted == 0) return
    if (PxlNative.surfaceBind(wanted, view.holder.surface) == 0) {
      bound = wanted
      onAttach(mapOf("surface" to wanted))
    }
  }
}
