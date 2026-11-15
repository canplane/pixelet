// @pxl/client/modules/platform/android/src/main/java/me/plei/pixelet/platform/Text.kt

package me.plei.pixelet.platform

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Typeface
import java.nio.ByteBuffer

// Text in the platform's own font, as the core asks for it
// (pxl-runtime text.rs), on the core's own thread: how wide a line
// runs, and a line drawn. The font is the system's, which falls back through every script
// and draws color emoji as the app's own text does. This file does on Android's
// text engine what Text.swift does on the iPhone's.

/** How far a line of `text` runs at a CSS weight and a size, in pixels. */
fun textWidth(text: String, weight: Double, px: Double): Double = paint(weight, px, 0xffffffffL).measureText(text).toDouble()

/** Draws a line of `text` in the color `rgba` (0xRRGGBBAA) into `out`,
 * starting at `x`, with the middle of its em box on the middle row. `out` is
 * `width` by `height` pixels of RGBA, top row first, with alpha not
 * premultiplied. */
fun textRaster(text: String, weight: Double, px: Double, rgba: Long, x: Double, width: Int, height: Int, out: ByteBuffer) {
  if (width <= 0 || height <= 0) return
  val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
  val paint = paint(weight, px, rgba)
  val metrics = paint.fontMetrics
  // Android counts down from the top row, and its ascent is negative.
  Canvas(bitmap).drawText(text, x.toFloat(), height / 2f - (metrics.ascent + metrics.descent) / 2f, paint)
  // A bitmap's pixels copy out as premultiplied RGBA bytes. The core reads
  // straight alpha, which is what a page's canvas always hands back, so the
  // colors are divided by alpha here and both platforms give the same.
  val bytes = ByteArray(width * height * 4)
  bitmap.copyPixelsToBuffer(java.nio.ByteBuffer.wrap(bytes))
  bitmap.recycle()
  for (i in bytes.indices step 4) {
    val a = bytes[i + 3].toInt() and 0xff
    if (a == 0 || a == 255) continue
    for (c in 0 until 3) {
      val v = bytes[i + c].toInt() and 0xff
      bytes[i + c] = minOf(255, (v * 255 + a / 2) / a).toByte()
    }
  }
  out.put(bytes)
}

/** The system font at a CSS weight (400 is regular, 600 semibold), a size and
 * a color, antialiased as the app's own text is. */
private fun paint(weight: Double, px: Double, rgba: Long): Paint = Paint(Paint.ANTI_ALIAS_FLAG or Paint.SUBPIXEL_TEXT_FLAG).apply {
  typeface = Typeface.create(Typeface.DEFAULT, weight.toInt().coerceIn(1, 1000), false)
  textSize = px.toFloat()
  val argb = ((rgba and 0xff) shl 24) or ((rgba ushr 8) and 0xffffff)
  color = argb.toInt()
}
