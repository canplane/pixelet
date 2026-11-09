// @pxl/app/modules/platform/android/src/main/java/me/plei/pixelet/platform/KeyboardDock.kt

package me.plei.pixelet.platform

import android.content.Context
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsAnimationCompat
import androidx.core.view.WindowInsetsCompat
import expo.modules.kotlin.AppContext
import expo.modules.kotlin.viewevent.EventDispatcher
import expo.modules.kotlin.views.ExpoView

/**
 * A view that stands on the keyboard (ts/client/src/ui/shared/component/above-keyboard.native.tsx).
 * It is the Android side of KeyboardDock.swift. The UI's layout places the
 * view where it rests with no keyboard up, `rest` off the bottom edge
 * (ts/client/src/ui/shared/component/bottom-rest.ts). While a keyboard is up, the
 * view is lifted to stand `clearance` above it. Android reports where the
 * keyboard is on every frame it moves, so the view moves with each report
 * rather than animating toward where the keyboard will end. It keeps to the
 * keys however they move, even when a finger drags them down.
 *
 * The view moves by a translation, which React Native never sees, because it
 * measures a view where its layout put it. On a translated view, a press
 * whose finger slides by a pixel is taken for one that has left the view, and
 * the send button does not send. So the translation is only ever the part of
 * the lift that the layout has not taken yet. Once the keyboard is still, the
 * view reports the lift (`onLift`). The UI then lays the view out that much
 * higher and reports it back (`layoutLift`), and the translation falls to
 * zero. At rest, the view is where the layout says it is.
 */
class KeyboardDock(context: Context, appContext: AppContext) : ExpoView(context, appContext) {
  private val onLift by EventDispatcher()

  /** The gap, in pixels, that the UI keeps from the bottom edge and from the
   * keyboard. */
  var clearance: Int = 0
    set(value) {
      field = value
      relift()
    }

  /** How far off the bottom edge the UI rests the view, in pixels. The lift
   * is measured from there, so that the rule for where the view rests stays
   * the UI's alone. */
  var rest: Int = 0
    set(value) {
      field = value
      relift()
    }

  /** How much of the lift the UI's layout has taken, in pixels. */
  var layoutLift: Int = 0
    set(value) {
      field = value
      relift()
    }

  /** The keyboard's height the view was last lifted for, in pixels. */
  private var keyboard = 0

  /** Whether the keyboard is moving. The lift is reported only once it
   * stops. */
  private var moving = false

  /** The lift last reported, so that a still keyboard is reported once. */
  private var said = -1

  init {
    // A LinearLayout clips its children to its bounds, and this view is only
    // as tall as what stands on it. A shadow drawn past a pill's body, such as
    // a toast's or the dock's, would be cut at this view's edge. Neither the
    // page nor the iPhone's view clips there.
    clipChildren = false
    clipToPadding = false
    ViewCompat.setWindowInsetsAnimationCallback(
      this,
      object : WindowInsetsAnimationCompat.Callback(DISPATCH_MODE_CONTINUE_ON_SUBTREE) {
        override fun onPrepare(animation: WindowInsetsAnimationCompat) {
          moving = true
        }

        override fun onProgress(insets: WindowInsetsCompat, running: MutableList<WindowInsetsAnimationCompat>): WindowInsetsCompat {
          follow(insets)
          return insets
        }

        override fun onEnd(animation: WindowInsetsAnimationCompat) {
          moving = false
          ViewCompat.getRootWindowInsets(this@KeyboardDock)?.let(::follow) ?: relift()
        }
      },
    )
    // Insets that arrive without an animation: the keyboard was already up
    // when the view appeared, or it moved without animating.
    ViewCompat.setOnApplyWindowInsetsListener(this) { _, insets ->
      follow(insets)
      insets
    }
  }

  private fun follow(insets: WindowInsetsCompat) {
    keyboard = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
    relift()
  }

  /** Translates the view by the part of the lift its layout has not taken,
   * and reports the lift once the keyboard is still. The report is in
   * density-independent pixels, the unit of React Native's layout. */
  private fun relift() {
    val lift = maxOf(0, keyboard + clearance - rest)
    translationY = -(lift - layoutLift).toFloat()
    if (!moving && lift != said) {
      said = lift
      onLift(mapOf("lift" to lift / resources.displayMetrics.density.toDouble()))
    }
  }
}
