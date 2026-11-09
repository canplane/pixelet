// @xpute/native/ios/XputeModule.swift

import CXpute
import ExpoModulesCore
import Foundation
import QuartzCore

/// This tells the core's thread each display frame (`xpute_vsync`), which
/// begins the turns. A display link calls it on the main thread.
final class DisplayFrames: NSObject {
  private var link: CADisplayLink?

  func start() {
    DispatchQueue.main.async {
      self.link = CADisplayLink(target: self, selector: #selector(self.frame))
      self.link?.add(to: .main, forMode: .common)
    }
  }

  func stop() {
    DispatchQueue.main.async {
      self.link?.invalidate()
      self.link = nil
    }
  }

  @objc private func frame(link: CADisplayLink) {
    xpute_vsync((CACurrentMediaTime() - link.timestamp) * 1000)
  }
}

/// This is xpute's native module on iOS (src/mod.ts). It reaches the core and
/// its GPU runtime through xpute's C ABI (xpute.h, xpute_gpu.h), and holds the
/// view the core's surface draws into (SurfaceView.swift). android/'s
/// XputeModule.kt is the same module under the same names.
public class XputeModule: Module {
  /// The function given to `xpute_notify_on` sends its event through this
  /// module.
  nonisolated(unsafe) static weak var current: XputeModule?
  /// The JavaScript function that answers the core's calls. It is called on
  /// the JavaScript thread, inside `run` or `answer`.
  nonisolated(unsafe) static var answer: JavaScriptValue?
  private let frames = DisplayFrames()

  public func definition() -> ModuleDefinition {
    Name("Xpute")

    /// Fires when frames wait for `take`, or when a call waits for `answer`.
    Events("onNotify")

    OnCreate {
      XputeModule.current = self
      self.frames.start()
      // A C function captures nothing, so it reaches the module through the
      // static. It is called from the core's thread, and Expo's sendEvent
      // itself schedules the event onto the JavaScript thread.
      xpute_notify_on { XputeModule.current?.sendEvent("onNotify") }
      // This runs on the JavaScript thread, inside `run` or `answer`, so it
      // may call into JavaScript. Without an answer it returns ENOSYS (-38),
      // and when the JavaScript answer throws, EIO (-5).
      xpute_call_on { at in
        guard let answer = XputeModule.answer else { return -38 }
        guard let res = try? answer.getFunction().call(arguments: Int(at)) else { return -5 }
        return Int32(truncatingIfNeeded: res.getInt())
      }
    }
    OnDestroy {
      self.frames.stop()
      XputeModule.current = nil
      XputeModule.answer = nil
      xpute_call_on(nil)
      xpute_notify_on(nil)
    }

    Function("mem") { () throws -> NativeArrayBuffer in
      var bytes: UInt32 = 0
      guard let at = xpute_mem(&bytes) else { throw MemoryException() }
      // The region belongs to the core for the app's whole life, so nothing
      // is freed when the buffer is.
      return try NativeArrayBuffer.wrap(dataWithoutCopy: UnsafeMutableRawBufferPointer(start: at, count: Int(bytes)), cleanup: {})
    }

    // A word crosses as a Double: every u32 fits in one, and Expo's fast path
    // takes it.
    Function("load") { (at: Double) -> Double in
      Double(xpute_load(UInt32(at)))
    }

    Function("store") { (at: Double, word: Double) in
      xpute_store(UInt32(at), UInt32(word))
    }

    Function("wake") {
      xpute_wake()
    }

    Function("input") {
      xpute_input()
    }

    Function("pause") {
      xpute_pause()
    }

    Function("resume") {
      xpute_resume()
    }

    Function("answer") {
      xpute_answer()
    }

    Function("attach") { (sqAt: Double, cqAt: Double, fs: Bool) in
      xpute_attach(UInt32(sqAt), UInt32(cqAt), fs)
    }

    Function("pace") { (paceHz: Double, marginShare: Double, batchFrames: Double, settleMs: Double) in
      xpute_pace(paceHz, marginShare, batchFrames, settleMs)
    }

    Function("post") { (tag: Double, cmd: Double, flags: Double, result: Double, pkt: Uint8Array) in
      xpute_post(
        UInt32(tag), UInt32(cmd), UInt32(flags), Int32(result),
        pkt.rawPointer.assumingMemoryBound(to: UInt8.self), pkt.byteLength)
    }

    Function("take") { () throws -> NativeArrayBuffer in
      var bytes = 0
      let at = xpute_take(&bytes)!
      return try NativeArrayBuffer.wrap(
        dataWithoutCopy: UnsafeMutableRawBufferPointer(start: at, count: bytes),
        cleanup: { xpute_taken_free(at, bytes) })
    }

    Function("fsOpened") { (fd: Double, result: Double, detail: Double, bytes: Uint8Array) in
      xpute_fs_opened(
        UInt32(fd), Int32(result), UInt32(detail),
        bytes.rawPointer.assumingMemoryBound(to: UInt8.self), bytes.byteLength)
    }

    Function("fsClosed") { (fd: Double, result: Double) in
      xpute_fs_closed(UInt32(fd), Int32(result))
    }

    Function("call") { (cmd: Double, pkt: Uint8Array) -> Double in
      Double(xpute_call(UInt32(cmd), pkt.rawPointer.assumingMemoryBound(to: UInt8.self), pkt.byteLength))
    }

    /// The answer is kept past the call, so a copy of the value is stored.
    Function("callOn") { (answer: JavaScriptValue) in
      XputeModule.answer = answer.copy()
    }

    Function("gpuOpen") { () -> Bool in
      xpute_gpu_open() == 0
    }

    Function("say") { (line: String) in
      FileHandle.standardOutput.write(Data((line + "\n").utf8))
    }

    View(SurfaceView.self) {
      Events("onAttach", "onDetach")
      Prop("surface") { (view: SurfaceView, surface: UInt32) in
        view.surface = surface
      }
    }
  }
}

internal final class MemoryException: Exception, @unchecked Sendable {
  override var reason: String {
    "the core has no memory"
  }
}
