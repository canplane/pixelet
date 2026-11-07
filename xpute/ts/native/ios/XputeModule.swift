// @xpute/native/ios/XputeModule.swift

import CXpute
import ExpoModulesCore

/// xpute's module on an iPhone (src/mod.ts): the core and its GPU runtime
/// through xpute's C ABI (xpute.h, xpute_gpu.h), and the view the core's
/// surface draws into (SurfaceView.swift). android/'s XputeModule.kt is the
/// same under the same names.
public class XputeModule: Module {
  /// The module `xpute_notify_on`'s function sends through.
  nonisolated(unsafe) static weak var current: XputeModule?
  /// What answers the core's calls, called on the JavaScript thread,
  /// inside `run` or `answer`.
  nonisolated(unsafe) static var answer: JavaScriptValue?

  public func definition() -> ModuleDefinition {
    Name("Xpute")

    /// The core has posted or asked, a call waits for `answer`, or the
    /// core has stopped.
    Events("onNotify")

    OnCreate {
      XputeModule.current = self
      // A C function captures nothing, so it reaches the module through the
      // static. Called from the core's thread: Expo's sendEvent schedules
      // the event onto the JavaScript thread itself.
      xpute_notify_on { XputeModule.current?.sendEvent("onNotify") }
      // On the JavaScript thread, inside `run` or `answer`, which may call
      // into JavaScript there. A runtime that throws refused it (EIO).
      xpute_call_on { at in
        guard let answer = XputeModule.answer else { return -38 }
        guard let res = try? answer.getFunction().call(arguments: Int(at)) else { return -5 }
        return Int32(truncatingIfNeeded: res.getInt())
      }
    }
    OnDestroy {
      XputeModule.current = nil
      XputeModule.answer = nil
      xpute_call_on(nil)
      xpute_notify_on(nil)
    }

    Function("mem") { () throws -> NativeArrayBuffer in
      var bytes: UInt32 = 0
      guard let at = xpute_mem(&bytes) else { throw MemoryException() }
      // The region is the core's for the app's life: nothing is let go when
      // the buffer is.
      return try NativeArrayBuffer.wrap(dataWithoutCopy: UnsafeMutableRawBufferPointer(start: at, count: Int(bytes)), cleanup: {})
    }

    Function("run") { (deadlineMs: Double) -> Double in
      xpute_run(deadlineMs)
    }

    Function("kick") { (deadlineMs: Double) in
      xpute_kick(deadlineMs)
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

    /// Kept past the call, so a copy of the value it was handed.
    Function("callOn") { (answer: JavaScriptValue) in
      XputeModule.answer = answer.copy()
    }

    Function("gpuOpen") { () -> Bool in
      xpute_gpu_open() == 0
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
