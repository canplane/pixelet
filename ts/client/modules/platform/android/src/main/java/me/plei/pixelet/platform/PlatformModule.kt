// @pxl/client/modules/platform/android/src/main/java/me/plei/pixelet/platform/PlatformModule.kt

package me.plei.pixelet.platform

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.hardware.Sensor
import android.hardware.SensorManager
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import expo.modules.kotlin.exception.CodedException
import expo.modules.kotlin.jni.NativeArrayBuffer
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

/**
 * The native half of the client's services on Android (the .native files in
 * ts/client/src/service/platform/). It answers what the UI asks of the system,
 * under the same names as the iPhone's PlatformModule.swift. The core is in
 * xpute's own module (@xpute/expo), and the view it draws into in @pxl/gpu/expo's.
 */
class PlatformModule : Module() {
  private val context: Context
    get() = appContext.reactContext ?: throw CodedException("the app has no context")

  override fun definition() = ModuleDefinition {
    Name("PxlPlatform")

    OnCreate { PxlNative.textOn() }

    // ---- the transport (service/platform/origin.native.ts) ----

    /** Decodes a body that arrived still zstd-compressed. It runs on the
     * module's queue, off the JavaScript thread, and decodes into a direct
     * buffer that JavaScript takes without a copy. */
    AsyncFunction("unpack") { packed: ByteArray ->
      NativeArrayBuffer(PxlNative.unpack(packed) ?: throw CodedException("a body sent zstd does not decode"))
    }

    // ---- storage (service/platform/storage.native.ts) ----
    //
    // SharedPreferences is the system's store for an app's small values, as
    // localStorage is the browser's for a page.

    Function("storage_get") { key: String -> preferences().getString(key, null) }

    Function("storage_keys") { preferences().all.keys.toList() }

    Function("storage_set") { key: String, value: String ->
      preferences().edit().putString(key, value).apply()
    }

    // ---- the clipboard (service/platform/share.native.ts) ----

    Function("clipboard_set") { text: String ->
      val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
      clipboard.setPrimaryClip(ClipData.newPlainText("pixelet", text))
    }

    // ---- the compass (service/platform/sensor.native.ts) ----

    /** Whether the device has a compass at all. expo-location tells
     * that only by failing a watch, and a failed watch reads as a compass lost
     * rather than one that was never there. */
    Function("has_heading") {
      val sensors = context.getSystemService(Context.SENSOR_SERVICE) as SensorManager
      sensors.getDefaultSensor(Sensor.TYPE_ROTATION_VECTOR) != null || sensors.getDefaultSensor(Sensor.TYPE_MAGNETIC_FIELD) != null
    }

    // ---- the safe area (service/platform/safe-area.native.ts) ----

    /** How much of the screen's bottom inset takes a touch, in
     * density-independent pixels. That is the bar of navigation buttons, or
     * nothing under gesture navigation, whose handle takes only a swipe. The
     * UI rests its bottom chrome clear of it
     * (ts/client/src/ui/shared/component/bottom-rest.ts). */
    Function("tappable_bottom") {
      val view = appContext.currentActivity?.window?.decorView ?: return@Function 0.0
      val insets = ViewCompat.getRootWindowInsets(view) ?: return@Function 0.0
      insets.getInsets(WindowInsetsCompat.Type.tappableElement()).bottom / view.resources.displayMetrics.density.toDouble()
    }

    // ---- the keyboard (ts/client/src/ui/shared/component/above-keyboard.native.tsx, KeyboardDock.kt) ----

    View(KeyboardDock::class) {
      Events("onLift")
      // The UI's lengths are in density-independent pixels, and the view's are
      // in the screen's pixels.
      Prop("clearance") { view: KeyboardDock, clearance: Double -> view.clearance = view.px(clearance) }
      Prop("rest") { view: KeyboardDock, rest: Double -> view.rest = view.px(rest) }
      Prop("layoutLiftPx") { view: KeyboardDock, layoutLiftDp: Double -> view.layoutLift = view.px(layoutLiftDp) }
    }
  }

  private fun preferences() = context.getSharedPreferences("pxl", Context.MODE_PRIVATE)

  private fun android.view.View.px(dp: Double): Int = (dp * resources.displayMetrics.density).toInt()
}
