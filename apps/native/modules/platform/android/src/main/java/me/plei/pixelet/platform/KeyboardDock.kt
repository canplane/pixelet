// @pxl/native/modules/platform/android/src/main/java/me/plei/pixelet/platform/KeyboardDock.kt

package me.plei.pixelet.platform

import android.content.Context
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsAnimationCompat
import androidx.core.view.WindowInsetsCompat
import expo.modules.kotlin.AppContext
import expo.modules.kotlin.views.ExpoView

/**
 * A view that stands on the keyboard (apps/native/src/platform/above-keyboard.tsx),
 * KeyboardDock.swift's on Android. Its layout is the shell's, where it stands
 * with no keyboard up: `clearance` from the bottom edge, or on the system's bars
 * where they are higher. While a keyboard is up, the view is lifted to stand
 * `clearance` above it. Android says where the keyboard is every frame it
 * moves, so the view is moved on each of them rather than animated towards
 * where the keyboard is going: it keeps to the keys however they move,
 * a finger dragging them down included.
 *
 * The lift is a translation on the view, so the layout the shell gives it and
 * the lift the keyboard gives it are two things that never overwrite each
 * other.
 */
class KeyboardDock(context: Context, appContext: AppContext) : ExpoView(context, appContext) {
  /** The gap the shell keeps from the bottom edge and from the keyboard, in
   * pixels. */
  var clearance: Int = 0
    set(value) {
      field = value
      ViewCompat.getRootWindowInsets(this)?.let(::lift)
    }

  init {
    // A LinearLayout clips what it holds to its bounds, and the dock is only
    // as tall as what stands on it: a shadow drawn past a pill's body — the
    // toasts', the mode bar's — was cut at the dock's edge. Neither the page
    // nor an iPhone's view clips there.
    clipChildren = false
    clipToPadding = false
    ViewCompat.setWindowInsetsAnimationCallback(
      this,
      object : WindowInsetsAnimationCompat.Callback(DISPATCH_MODE_CONTINUE_ON_SUBTREE) {
        override fun onProgress(insets: WindowInsetsCompat, running: MutableList<WindowInsetsAnimationCompat>): WindowInsetsCompat {
          lift(insets)
          return insets
        }
      },
    )
    // Where the keyboard is when nothing is animating — it was up when the
    // view came, or it moved without an animation.
    ViewCompat.setOnApplyWindowInsetsListener(this) { _, insets ->
      lift(insets)
      insets
    }
  }

  private fun lift(insets: WindowInsetsCompat) {
    val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
    val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars()).bottom
    translationY = -maxOf(0, keyboard + clearance - maxOf(clearance, bars)).toFloat()
  }
}
