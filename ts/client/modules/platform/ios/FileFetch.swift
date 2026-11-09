// @pxl/native/modules/platform/ios/FileFetch.swift

import ExpoModulesCore
import Foundation
import PxlNative

/// Fetches the files the core opens off the JavaScript thread
/// (ts/client/src/service/platform/file.native.ts). The bytes go straight to
/// the runtime; JavaScript hears only what kept them from it.
final class FileFetch {
  private let lock = NSLock()
  private var tasks: [UInt32: URLSessionDataTask] = [:]
  /// The descriptors cancelled before their fetch began: `fetch` runs on the
  /// module's queue and `cancel` on the JavaScript thread. A descriptor is not
  /// used twice, so one left here harms nothing.
  private var cancelled: Set<UInt32> = []

  func fetch(_ fd: UInt32, _ url: URL, _ promise: Promise) {
    var request = URLRequest(url: url)
    request.setValue("zstd", forHTTPHeaderField: "accept-encoding")
    var task: URLSessionDataTask!
    task = URLSession.shared.dataTask(with: request) { [self] data, response, error in
      lock.lock()
      let current = tasks[fd] === task
      if current { tasks[fd] = nil }
      lock.unlock()
      // A file closed before its answer came, or its descriptor since reused.
      guard current else { return promise.resolve(0) }
      if let error { return promise.reject("ERR_FILE_FETCH", error.localizedDescription) }
      guard let http = response as? HTTPURLResponse else {
        return promise.reject("ERR_FILE_FETCH", "no HTTP response")
      }
      guard (200..<300).contains(http.statusCode), http.statusCode != 204, let data else {
        return promise.resolve(http.statusCode)
      }
      let zstd = http.value(forHTTPHeaderField: "content-encoding") == "zstd"
      let good = data.withUnsafeBytes { body in
        pxl_file_body(fd, body.bindMemory(to: UInt8.self).baseAddress, body.count, zstd)
      }
      if good {
        promise.resolve(0)
      } else {
        promise.reject("ERR_FILE_FETCH", "the body does not decode")
      }
    }
    lock.lock()
    let wanted = cancelled.remove(fd) == nil
    if wanted { tasks[fd] = task }
    lock.unlock()
    guard wanted else { return promise.resolve(0) }
    task.resume()
  }

  func cancel(_ fd: UInt32) {
    lock.lock()
    let task = tasks.removeValue(forKey: fd)
    if task == nil { cancelled.insert(fd) }
    lock.unlock()
    task?.cancel()
  }
}
