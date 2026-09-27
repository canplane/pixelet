// @pxl/native/modules/platform/android/src/main/java/me/plei/pixelet/platform/Text.kt

package me.plei.pixelet.platform

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Typeface
import expo.modules.kotlin.typedarray.Uint8Array

// Text in the platform's own family, as the kernel asks for it
// (apps/native/src/platform/text.ts): how wide a line runs, and a line drawn.
// The system font, which falls back through every script and draws color
// emoji as the app's own text does — Text.swift's, on Android's engine.

/** How far a line of `text` runs at a CSS weight and a size, in pixels. */
fun textWidth(text: String, weight: Double, px: Double): Double = paint(weight, px, 0xffffffffL).measureText(text).toDouble()

/** A line of `text` in `rgba` (0xRRGGBBAA), starting at `x` with the middle
 * of its em box on the middle row, into `out`: `width` by `height` pixels,
 * RGBA, top row first, alpha not premultiplied. */
fun textRaster(text: String, weight: Double, px: Double, rgba: Long, x: Double, width: Int, height: Int, out: Uint8Array) {
  if (width <= 0 || height <= 0) return
  val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
  val paint = paint(weight, px, rgba)
  val metrics = paint.fontMetrics
  // Android counts down from the top row, and its ascent is negative.
  Canvas(bitmap).drawText(text, x.toFloat(), height / 2f - (metrics.ascent + metrics.descent) / 2f, paint)
  // A bitmap's pixels copy out as RGBA bytes, premultiplied; the kernel reads
  // straight alpha, which a page's canvas hands back whatever it drew in, so
  // straight is what both platforms give.
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
  out.write(bytes, 0, bytes.size)
}

/** The system font at a CSS weight (400 regular, 600 semibold), a size and a
 * color, antialiased as the app's own text is. */
private fun paint(weight: Double, px: Double, rgba: Long): Paint = Paint(Paint.ANTI_ALIAS_FLAG or Paint.SUBPIXEL_TEXT_FLAG).apply {
  typeface = Typeface.create(Typeface.DEFAULT, weight.toInt().coerceIn(1, 1000), false)
  textSize = px.toFloat()
  val argb = ((rgba and 0xff) shl 24) or ((rgba ushr 8) and 0xffffff)
  color = argb.toInt()
}
