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
 * The device's half of the app's platform (apps/native/src/platform/) on
 * Android, the module an iPhone's PlatformModule.swift is: the kernel as the
 * shell reaches it, the view it draws into, and what the shell asks of the
 * system, under the same names.
 *
 * The kernel's memory is handed over as one ArrayBuffer over the region the
 * core took — no copy, so a ring the shell writes is the ring the kernel
 * reads. The shell calls on the JavaScript thread, one call at a time; the
 * core runs each on its own thread while this one waits
 * (crates/pxl/web/native/src/thread.rs), and an image's decode runs off both.
 */
class PlatformModule : Module() {
  private val context: Context
    get() = appContext.reactContext ?: throw CodedException("the app has no context")

  override fun definition() = ModuleDefinition {
    Name("PxlPlatform")

    // ---- the kernel (platform/core.ts, gpu.ts, image.ts) ----

    // The region is the core's for the app's life: nothing is let go when
    // the buffer is.
    Function("memory") { NativeArrayBuffer(PxlCore.memory()) }

    Function("interrupt") { quotaMs: Double -> PxlCore.interrupt(quotaMs) }

    /** The device's uniform offset alignment and whether it times its
     * passes, or null where the system has no GPU. */
    Function("gpuOpen") { PxlCore.gpuOpen()?.toList() }

    /** Decodes a PNG or a WebP and holds it for the renderer: its handle, or 0
     * when it does not decode. Async, so it runs off the JavaScript thread: a
     * large image costs no turn. */
    AsyncFunction("imageHold") { file: ByteArray -> PxlCore.imageHold(file) }

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

    /** Whether the device has a compass at all, which expo-location only says
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
     * rests its bottom chrome clear of (shared/ui/bottom-rest.ts). */
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
      Prop("clearance") { view: KeyboardDock, clearance: Double ->
        view.clearance = (clearance * view.resources.displayMetrics.density).toInt()
      }
      Prop("rest") { view: KeyboardDock, rest: Double ->
        view.rest = (rest * view.resources.displayMetrics.density).toInt()
      }
      Prop("applied") { view: KeyboardDock, applied: Double ->
        view.applied = (applied * view.resources.displayMetrics.density).toInt()
      }
    }
  }

  private fun preferences() = context.getSharedPreferences("pxl", Context.MODE_PRIVATE)
}
