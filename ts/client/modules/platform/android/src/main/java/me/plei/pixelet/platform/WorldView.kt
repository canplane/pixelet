// @pxl/app/modules/platform/android/src/main/java/me/plei/pixelet/platform/WorldView.kt

package me.plei.pixelet.platform

import android.content.Context
import android.view.MotionEvent
import expo.modules.kotlin.AppContext
import expo.modules.kotlin.viewevent.EventDispatcher
import xpute.SurfaceView

/** The world's surface, which hands its touches to the UI as pointer events
 * (`onPointer`), as a page's canvas gives its pointer events. */
class WorldView(context: Context, appContext: AppContext) : SurfaceView(context, appContext) {
  private val onPointer by EventDispatcher()
  private val density = context.resources.displayMetrics.density

  init {
    view.setOnTouchListener { _, event -> tell(event) }
  }

  private fun send(phase: String, event: MotionEvent, index: Int) {
    onPointer(mapOf(
      "phase" to phase,
      "id" to event.getPointerId(index),
      "x" to (event.getX(index) / density).toDouble(),
      "y" to (event.getY(index) / density).toDouble(),
    ))
  }

  private fun tell(event: MotionEvent): Boolean {
    val at = event.actionIndex
    when (event.actionMasked) {
      MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> send("down", event, at)
      MotionEvent.ACTION_MOVE -> for (i in 0 until event.pointerCount) send("move", event, i)
      // A lifted finger also leaves the surface, as a page's pointerleave
      // follows its pointerup, so that nothing stays hovered under it.
      MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> {
        send("up", event, at)
        send("out", event, at)
      }
      MotionEvent.ACTION_CANCEL -> for (i in 0 until event.pointerCount) {
        send("cancel", event, i)
        send("out", event, i)
      }
    }
    return true
  }
}
