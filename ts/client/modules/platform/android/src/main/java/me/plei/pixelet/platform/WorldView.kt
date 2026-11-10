// @pxl/app/modules/platform/android/src/main/java/me/plei/pixelet/platform/WorldView.kt

package me.plei.pixelet.platform

import android.content.Context
import android.view.MotionEvent
import expo.modules.kotlin.AppContext
import xpute.SurfaceView

/** The world's surface, which tells the core its touches (`PxlNative.touch`). */
class WorldView(context: Context, appContext: AppContext) : SurfaceView(context, appContext) {
  private val density = context.resources.displayMetrics.density

  init {
    view.setOnTouchListener { _, event -> tell(event) }
  }

  private fun send(phase: Int, event: MotionEvent, index: Int) {
    PxlNative.touch(phase, event.getPointerId(index), (event.getX(index) / density).toDouble(), (event.getY(index) / density).toDouble())
  }

  private fun tell(event: MotionEvent): Boolean {
    val at = event.actionIndex
    when (event.actionMasked) {
      MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> send(0, event, at)
      MotionEvent.ACTION_MOVE -> for (i in 0 until event.pointerCount) send(1, event, i)
      // A lifted finger also leaves the surface, as a page's pointerleave
      // follows its pointerup, so that nothing stays hovered under it.
      MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> {
        send(2, event, at)
        send(3, event, at)
      }
      MotionEvent.ACTION_CANCEL -> for (i in 0 until event.pointerCount) {
        send(4, event, i)
        send(3, event, i)
      }
    }
    return true
  }
}
