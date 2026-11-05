// @xpute/native/ios/XputeModule.swift

import CXpute
import ExpoModulesCore

/// xpute's module on an iPhone (src/mod.ts): the guest and its GPU host
/// through xpute's C ABI (xpute.h, xpute_gpu.h), and the view the guest's
/// surface draws into (SurfaceView.swift). android/'s XputeModule.kt is the
/// same under the same names.
public class XputeModule: Module {
  /// The module `xpute_gpu_wake_on`'s function sends through.
  nonisolated(unsafe) static weak var current: XputeModule?
  /// What answers the guest's hostcalls, called on the JavaScript thread
  /// while `run` waits on the turn making them.
  nonisolated(unsafe) static var answer: JavaScriptValue?

  public func definition() -> ModuleDefinition {
    Name("Xpute")

    /// The GPU host asks a turn, for a completion it learned of between turns.
    Events("onWake")

    OnCreate {
      XputeModule.current = self
      // A C function captures nothing, so it reaches the module through the
      // static. Called from the thread that learned of a completion: Expo's
      // sendEvent schedules the event onto the JavaScript thread itself.
      xpute_gpu_wake_on { XputeModule.current?.sendEvent("onWake") }
      // On the thread waiting in `run`, the JavaScript thread, which may
      // call into JavaScript there. A host that throws refused it (EIO).
      xpute_hostcall_on { at in
        guard let answer = XputeModule.answer else { return -38 }
        guard let res = try? answer.getFunction().call(arguments: Int(at)) else { return -5 }
        return Int32(truncatingIfNeeded: res.getInt())
      }
    }
    OnDestroy {
      XputeModule.current = nil
      XputeModule.answer = nil
      xpute_hostcall_on(nil)
    }

    Function("mem") { () throws -> NativeArrayBuffer in
      var bytes: UInt32 = 0
      guard let at = xpute_mem(&bytes) else { throw MemoryException() }
      // The region is the guest's for the app's life: nothing is let go when
      // the buffer is.
      return try NativeArrayBuffer.wrap(dataWithoutCopy: UnsafeMutableRawBufferPointer(start: at, count: Int(bytes)), cleanup: {})
    }

    Function("run") { (quotaMs: Double) -> Double in
      xpute_run(quotaMs)
    }

    /// Kept past the call, so a copy of the value it was handed.
    Function("hostcallOn") { (answer: JavaScriptValue) in
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
    "the guest has no memory"
  }
}
