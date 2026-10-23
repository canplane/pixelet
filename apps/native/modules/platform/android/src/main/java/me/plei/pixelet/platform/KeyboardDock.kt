// @pxl/native/modules/platform/android/src/main/java/me/plei/pixelet/platform/KeyboardDock.kt

package me.plei.pixelet.platform

import android.content.Context
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsAnimationCompat
import androidx.core.view.WindowInsetsCompat
import expo.modules.kotlin.AppContext
import expo.modules.kotlin.viewevent.EventDispatcher
import expo.modules.kotlin.views.ExpoView

/**
 * A view that stands on the keyboard (apps/client/src/shell/ui/above-keyboard.native.tsx),
 * KeyboardDock.swift's on Android. Its layout is the shell's, where it rests
 * with no keyboard up, `rest` off the bottom edge (apps/client/src/shell/ui/bottom-rest.ts).
 * While a keyboard is up, the view is lifted to stand `clearance` above it.
 * Android says where the keyboard is every frame it moves, so the view is
 * moved on each of them rather than animated towards where the keyboard is
 * going: it keeps to the keys however they move, a finger dragging them down
 * included.
 *
 * The moving is a translation, which React Native never sees: it measures a
 * view where its layout put it, and a press that slid a pixel on a view
 * lifted where no measure looks was taken for one that had left it, so the
 * send never sent. So the translation is only ever the part of the lift the
 * layout has not taken yet. Once the keyboard is still, the lift is said
 * (`onLift`), the shell lays the view out that much higher and says so
 * (`applied`), and the translation is none: at rest, the view is where the
 * layout says it is.
 */
class KeyboardDock(context: Context, appContext: AppContext) : ExpoView(context, appContext) {
  private val onLift by EventDispatcher()

  /** The gap the shell keeps from the bottom edge and from the keyboard, in
   * pixels. */
  var clearance: Int = 0
    set(value) {
      field = value
      relift()
    }

  /** How far off the bottom edge the shell rests the view, in pixels; the
   * lift is measured from there, so the rule of where it rests is the
   * shell's alone. */
  var rest: Int = 0
    set(value) {
      field = value
      relift()
    }

  /** How much of the lift the shell's layout carries, in pixels. */
  var applied: Int = 0
    set(value) {
      field = value
      relift()
    }

  /** The keyboard's height the view was last lifted for, in pixels. */
  private var keyboard = 0

  /** Whether the keyboard is moving, its end not yet the lift to say. */
  private var moving = false

  /** The lift last said, so a still keyboard is said once. */
  private var said = -1

  init {
    // A LinearLayout clips what it holds to its bounds, and this view is only
    // as tall as what stands on it: a shadow drawn past a pill's body — the
    // toasts', the dock's — was cut at this view's edge. Neither the page
    // nor an iPhone's view clips there.
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
    // Where the keyboard is when nothing is animating — it was up when the
    // view came, or it moved without an animation.
    ViewCompat.setOnApplyWindowInsetsListener(this) { _, insets ->
      follow(insets)
      insets
    }
  }

  private fun follow(insets: WindowInsetsCompat) {
    keyboard = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
    relift()
  }

  /** Moves the view by what of the lift its layout does not carry, and says
   * the lift once the keyboard is still. */
  private fun relift() {
    val lift = maxOf(0, keyboard + clearance - rest)
    translationY = -(lift - applied).toFloat()
    if (!moving && lift != said) {
      said = lift
      onLift(mapOf("lift" to lift / resources.displayMetrics.density.toDouble()))
    }
  }
}
