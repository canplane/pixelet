// @xpute/gpu-native-runtime/android/src/main/java/xpute/SurfaceView.kt

package xpute

import android.content.Context
import android.view.SurfaceHolder
import expo.modules.kotlin.AppContext
import expo.modules.kotlin.viewevent.EventDispatcher
import expo.modules.kotlin.views.ExpoView

/**
 * A guest's surface draws into this view: the Android counterpart of
 * ios/SurfaceView.swift. It holds a system SurfaceView, a window of its own
 * that the system composes under the app's views. The view binds the surface
 * the app names (`surface`) while the system's surface exists, and never past
 * the moment the system takes it back: `surfaceDestroyed` does not return until
 * the guest's GPU host has let the surface go. The view tells when the
 * surface is bound (`onAttach`) and when it no longer is (`onDetach`). The
 * guest sizes the surface and draws every frame inside an entry, so nothing
 * here draws, and the view takes no input: a program that wants the system
 * view's touches subclasses it.
 */
open class SurfaceView(context: Context, appContext: AppContext) : ExpoView(context, appContext), SurfaceHolder.Callback {
  private val onAttach by EventDispatcher()
  private val onDetach by EventDispatcher()

  protected val view = android.view.SurfaceView(context).also {
    it.holder.addCallback(this)
    addView(it)
  }

  /** The guest's surface the app wants shown here; 0 for none. */
  var surface: Int = 0
    set(value) {
      field = value
      rebind()
    }

  /** This says whether the system's surface exists to be drawn into. */
  private var ready = false

  /** The guest's surface bound to the system's surface; 0 for none. */
  private var bound = 0

  // React Native lays out this view but none of its children, so the
  // system's view is given the whole view here.
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

  private fun rebind() {
    val wanted = if (ready) surface else 0
    if (bound == wanted) return
    if (bound != 0) {
      Gpu.surfaceUnbind(bound)
      bound = 0
      onDetach(mapOf<String, Any>())
    }
    if (wanted == 0) return
    if (Gpu.surfaceBind(wanted, view.holder.surface) == 0) {
      bound = wanted
      onAttach(mapOf("surface" to wanted))
    }
  }
}
