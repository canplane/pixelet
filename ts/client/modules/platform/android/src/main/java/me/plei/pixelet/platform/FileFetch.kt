// @pxl/app/modules/platform/android/src/main/java/me/plei/pixelet/platform/FileFetch.kt

package me.plei.pixelet.platform

import android.content.Context
import com.facebook.react.modules.network.OkHttpClientProvider
import expo.modules.kotlin.Promise
import java.io.File
import java.io.IOException
import okhttp3.Cache
import okhttp3.Call
import okhttp3.Callback
import okhttp3.Request
import okhttp3.Response

/**
 * Fetches the files the core opens off the JavaScript thread
 * (ts/client/src/service/platform/file.native.ts). The bytes go straight to the
 * runtime; JavaScript hears only what kept them from it. Its calls to a host
 * are what the core's credits count (pxl-core sys/io/param.rs `CREDITS`).
 */
class FileFetch(context: Context) {
  private val client = OkHttpClientProvider.createClientBuilder()
    .cache(Cache(File(context.cacheDir, "file-fetch"), 10L * 1024 * 1024))
    .build()

  private val calls = HashMap<Int, Call>()

  /** The descriptors cancelled before their fetch began: `fetch` runs on the
   * module's queue and `cancel` on the JavaScript thread. A descriptor is not
   * used twice, so one left here harms nothing. */
  private val cancelled = HashSet<Int>()

  /** Takes the fetch of `fd` if `call` is still it. */
  @Synchronized private fun take(fd: Int, call: Call): Boolean = calls.remove(fd, call)

  @Synchronized private fun begin(fd: Int, call: Call): Boolean {
    if (cancelled.remove(fd)) return false
    calls[fd] = call
    return true
  }

  fun fetch(fd: Int, url: String, promise: Promise) {
    val request = Request.Builder().url(url).header("accept-encoding", "zstd").build()
    val call = client.newCall(request)
    if (!begin(fd, call)) return promise.resolve(0)
    call.enqueue(object : Callback {
      override fun onFailure(call: Call, e: IOException) {
        // A file closed before its answer came.
        if (!take(fd, call)) return promise.resolve(0)
        promise.reject("ERR_FILE_FETCH", e.message, e)
      }

      override fun onResponse(call: Call, response: Response) {
        response.use {
          if (!it.isSuccessful || it.code == 204) {
            return if (take(fd, call)) promise.resolve(it.code) else promise.resolve(0)
          }
          // It stays cancellable while its body comes.
          val body = try {
            it.body!!.bytes()
          } catch (e: IOException) {
            if (!take(fd, call)) return promise.resolve(0)
            return promise.reject("ERR_FILE_FETCH", e.message, e)
          }
          if (!take(fd, call)) return promise.resolve(0)
          if (PxlNative.fileBody(fd, body, it.header("content-encoding") == "zstd")) {
            promise.resolve(0)
          } else {
            promise.reject("ERR_FILE_FETCH", "the body does not decode", null)
          }
        }
      }
    })
  }

  @Synchronized fun cancel(fd: Int) {
    val call = calls.remove(fd)
    if (call != null) call.cancel() else cancelled.add(fd)
  }
}
