// @xpute/native/ios/XputeModule.swift

import CXpute
import ExpoModulesCore
import Foundation
import QuartzCore

/// This tells the core's thread each display frame (`xpute_vsync`). A
/// display link calls it on the main thread.
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

    /// Fires when the core has posted or asked for a turn, when a call
    /// waits for `answer`, or when the core has stopped.
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

    Function("run") { (deadlineMs: Double) -> Double in
      xpute_run(deadlineMs)
    }

    Function("kick") { (deadlineMs: Double, atMs: Double) in
      xpute_kick(deadlineMs, atMs)
    }

    Function("nextFrame") { (afterMs: Double) -> Double in
      xpute_next_frame(afterMs)
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

    Function("turns") { () -> [Double] in
      var turnMs: Double = 0
      let wake = xpute_turns(&turnMs)
      return [wake, turnMs]
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
