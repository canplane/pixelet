// @pxl/native/modules/platform/android/src/main/java/me/plei/pixelet/platform/PlatformModule.kt

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
import expo.modules.kotlin.typedarray.Uint8Array

/**
 * The device's half of the app's platform (apps/native/src/) on
 * Android, the module an iPhone's PlatformModule.swift is: the kernel as the
 * shell reaches it, the view it draws into, and what the shell asks of the
 * system, under the same names.
 *
 * The kernel's memory is handed over as one ArrayBuffer over the region the
 * guest took — no copy, so a ring the shell writes is the ring the kernel
 * reads. The shell calls on the JavaScript thread, one call at a time; the
 * guest runs each on its own thread while this one waits
 * (crates/client-bracket/native/src/lib.rs `on_kernel`), and an image's decode runs off both.
 */
class PlatformModule : Module() {
  private val context: Context
    get() = appContext.reactContext ?: throw CodedException("the app has no context")

  override fun definition() = ModuleDefinition {
    Name("PxlPlatform")

    // ---- the kernel (platform/guest.ts, gpu.ts) ----

    // The region is the guest's for the app's life: nothing is let go when
    // the buffer is.
    Function("memory") { NativeArrayBuffer(PxlGuest.memory()) }

    Function("interrupt") { quotaMs: Double -> PxlGuest.interrupt(quotaMs) }

    /** Whether the device opened; what it is, the kernel reads from the host. */
    Function("gpuOpen") { PxlGuest.gpuOpen() }

    // ---- the kernel's turns (platform/gpu.ts) ----

    /** The device's host asks a turn: a completion it writes at the next
     * turn's poll would otherwise wait for an input to be read. Sent from the
     * thread that learned of the completion; Expo carries the event to the
     * JavaScript thread itself. */
    Events("onWake")

    OnCreate { PxlGuest.woken = { sendEvent("onWake") } }

    OnDestroy { PxlGuest.woken = null }

    // ---- the transport (platform/origin.ts) ----

    /** A body fetched still zstd, decoded off the JavaScript thread on the
     * module's queue, into a direct buffer JavaScript takes without a copy. */
    AsyncFunction("unpack") { packed: ByteArray ->
      NativeArrayBuffer(PxlGuest.unpack(packed) ?: throw CodedException("a body sent zstd does not decode"))
    }

    // ---- storage (platform/storage.ts) ----
    //
    // SharedPreferences, the system's store for an app's small values, as a
    // page's localStorage is the browser's.

    Function("storageGet") { key: String -> preferences().getString(key, null) }

    Function("storageSet") { key: String, value: String ->
      preferences().edit().putString(key, value).apply()
    }

    // ---- the clipboard (platform/save.ts) ----

    Function("clipboardSet") { text: String ->
      val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
      clipboard.setPrimaryClip(ClipData.newPlainText("pixelet", text))
    }

    // ---- text (platform/text.ts, Text.kt) ----

    Function("textWidth") { weight: Double, px: Double, text: String -> textWidth(text, weight, px) }

    /** The line starting at `x` with the middle of its em box on the middle
     * row, into `out`: `width` by `height` pixels, RGBA, top row first, alpha
     * not premultiplied. */
    Function("textRaster") { weight: Double, px: Double, rgba: Double, text: String, x: Double, width: Int, height: Int, out: Uint8Array ->
      if (out.byteLength >= width * height * 4) textRaster(text, weight, px, rgba.toLong(), x, width, height, out)
    }

    // ---- the compass (platform/sensor.ts) ----

    /** Whether the device has a compass at all, which expo-horizon-location only says
     * by failing a watch — and a failed watch reads as a compass lost, not
     * one never there. */
    Function("headingAvailable") {
      val sensors = context.getSystemService(Context.SENSOR_SERVICE) as SensorManager
      sensors.getDefaultSensor(Sensor.TYPE_ROTATION_VECTOR) != null || sensors.getDefaultSensor(Sensor.TYPE_MAGNETIC_FIELD) != null
    }

    // ---- the safe area (platform/safe-area.ts) ----

    /** How much of the screen's bottom inset takes a touch, in
     * density-independent pixels: the navigation buttons' bar, or none with
     * gesture navigation, whose handle only takes a swipe. What the shell
     * rests its bottom chrome clear of (apps/client/src/shell/ui/bottom-rest.ts). */
    Function("tappableBottom") {
      val view = appContext.currentActivity?.window?.decorView ?: return@Function 0.0
      val insets = ViewCompat.getRootWindowInsets(view) ?: return@Function 0.0
      insets.getInsets(WindowInsetsCompat.Type.tappableElement()).bottom / view.resources.displayMetrics.density.toDouble()
    }

    // ---- the surface (platform/surface.tsx, KernelSurfaceView.kt) ----
    //
    // The first view, the one the module's name alone reaches.

    View(KernelSurfaceView::class) {
      Events("onAttach", "onDetach")
      Prop("surface") { view: KernelSurfaceView, surface: Int -> view.surface = surface }
    }

    // ---- the keyboard (platform/above-keyboard.tsx, KeyboardDock.kt) ----

    View(KeyboardDock::class) {
      Events("onLift")
      // The shell's lengths are in density-independent pixels, the view's in
      // the screen's.
      Prop("clearance") { view: KeyboardDock, clearance: Double -> view.clearance = view.px(clearance) }
      Prop("rest") { view: KeyboardDock, rest: Double -> view.rest = view.px(rest) }
      Prop("applied") { view: KeyboardDock, applied: Double -> view.applied = view.px(applied) }
    }
  }

  private fun preferences() = context.getSharedPreferences("pxl", Context.MODE_PRIVATE)

  private fun android.view.View.px(dp: Double): Int = (dp * resources.displayMetrics.density).toInt()
}
