// @xpute/expo/ios/XputeModule.swift

import CXpute
import ExpoModulesCore
import Foundation

/// This is xpute's native module on iOS (src/xpute.ts), over xpute's C ABI
/// (xpute.h): it starts the runtime on the guest's thread, carries the UI's
/// link to it and back, and tells it of the app's lifecycle itself.
/// android/'s XputeModule.kt is the same module under the same names.
public class XputeModule: Module {
  /// The functions given to `xpute_notify_on`, `xpute_stopped_on` and
  /// `xpute_trap_on` send their events through this module, the newest made.
  /// A reload may make the new module before the old one is destroyed, so a
  /// module clears the functions only while it is the current one, under
  /// `lock`.
  nonisolated(unsafe) static weak var current: XputeModule?
  private static let lock = NSLock()
  /// The UI's handle on the runtime, which its start answers and each post,
  /// answer and take names, so an old JavaScript runtime's reaches nothing
  /// once a new one has started (xpute.h `xpute_start`); 0, no UI's, until
  /// then.
  private var ui: UInt32 = 0

  /// The runtime's `len` bytes of UTF-8 at `at`.
  private static func text(_ at: UnsafePointer<CChar>?, _ len: Int) -> String {
    String(decoding: UnsafeRawBufferPointer(start: at, count: len), as: UTF8.self)
  }

  /// The guest's stop as `onStopped` carries it.
  private static func report(
    _ errnum: Int32, _ file: UnsafePointer<CChar>?, _ fileLen: Int, _ line: UInt32, _ n1: UInt64,
    _ n2: UInt64
  ) -> [String: Any] {
    [
      "errnum": Int(errnum),
      "file": text(file, fileLen),
      "line": Int(line),
      // A JavaScript number holds no u64 whole, so each crosses as its
      // decimal digits.
      "n1": String(n1),
      "n2": String(n2),
    ]
  }

  public func definition() -> ModuleDefinition {
    Name("Xpute")

    /// `onNotify` fires when a batch waits for `take`, `onStopped` when the
    /// guest stopped, with its report, and `onTrap` when it trapped, with what
    /// stopped it.
    Events("onNotify", "onStopped", "onTrap")

    OnCreate {
      XputeModule.lock.lock()
      defer { XputeModule.lock.unlock() }
      XputeModule.current = self
      // A C function captures nothing, so it reaches the module through the
      // static. Expo's sendEvent carries the event to the JavaScript thread
      // from any thread.
      xpute_notify_on { XputeModule.current?.sendEvent("onNotify") }
      xpute_stopped_on { errnum, file, fileLen, line, n1, n2 in
        XputeModule.current?.sendEvent(
          "onStopped", XputeModule.report(errnum, file, fileLen, line, n1, n2))
      }
      xpute_trap_on { err, errLen in
        XputeModule.current?.sendEvent("onTrap", ["err": XputeModule.text(err, errLen)])
      }
    }
    OnDestroy {
      XputeModule.lock.lock()
      defer { XputeModule.lock.unlock() }
      guard XputeModule.current === self else { return }
      XputeModule.current = nil
      xpute_notify_on(nil)
      xpute_stopped_on(nil)
      xpute_trap_on(nil)
    }

    OnAppEntersBackground {
      xpute_life(0)
    }

    OnAppEntersForeground {
      xpute_life(1)
    }

    Function("start") { (named: [UInt32], prefixes: Uint8Array) throws in
      // The directory crosses as its UTF-8 bytes, without the NUL.
      let caches = Array(try FileManager.default.url(
        for: .cachesDirectory, in: .userDomainMask, appropriateFor: nil, create: true
      ).path.utf8CString.dropLast())
      self.ui = named.withUnsafeBufferPointer { buf in
        xpute_start(
          buf.baseAddress, buf.count,
          prefixes.rawPointer.assumingMemoryBound(to: UInt8.self), prefixes.byteLength,
          caches, caches.count)
      }
    }

    Function("post") { (tag: UInt32, h: UInt32, m: UInt32, pkt: Uint8Array?) in
      xpute_post(
        self.ui, tag, h, m, pkt?.rawPointer.assumingMemoryBound(to: UInt8.self), pkt?.byteLength ?? 0)
    }

    Function("answer") { (tag: UInt32, result: Int32, data: Uint8Array?) in
      xpute_answer(
        self.ui, tag, result, data?.rawPointer.assumingMemoryBound(to: UInt8.self), data?.byteLength ?? 0)
    }

    /// The batch is JavaScript's without a copy, and freed when the buffer is.
    Function("take") { () throws -> NativeArrayBuffer in
      var len = 0
      let at = xpute_take(self.ui, &len)
      return try NativeArrayBuffer.wrap(
        dataWithoutCopy: UnsafeMutableRawBufferPointer(start: at, count: len),
        cleanup: { xpute_taken_free(at, len) })
    }

    /// A span the guest lent, its address in two words, `lo` and `hi`: the
    /// buffer is the UI's until the lend's answer, and nothing is freed when
    /// the buffer is.
    Function("bytes") { (lo: UInt32, hi: UInt32, len: Int) throws -> NativeArrayBuffer in
      let at = UnsafeMutableRawPointer(bitPattern: UInt(hi) << 32 | UInt(lo))
      return try NativeArrayBuffer.wrap(
        dataWithoutCopy: UnsafeMutableRawBufferPointer(start: at, count: len), cleanup: {})
    }

    /// The guest's last stop, as `onStopped` carries it, or nil before the
    /// guest stopped (xpute.h `xpute_stop`).
    Function("stop") { () -> [String: Any]? in
      var errnum: Int32 = 0
      var fileLen = 0
      var line: UInt32 = 0
      var n1: UInt64 = 0
      var n2: UInt64 = 0
      guard let file = xpute_stop(&errnum, &fileLen, &line, &n1, &n2) else { return nil }
      return XputeModule.report(errnum, file, fileLen, line, n1, n2)
    }

    /// What trapped the guest, or nil before it trapped (xpute.h
    /// `xpute_trap`).
    Function("trap") { () -> String? in
      var len = 0
      guard let err = xpute_trap(&len) else { return nil }
      return XputeModule.text(err, len)
    }

    // Each string crosses as its UTF-8 bytes, without the NUL.
    Function("log") { (level: UInt32, context: String, message: String) in
      let context = Array(context.utf8CString.dropLast())
      let message = Array(message.utf8CString.dropLast())
      xpute_log(level, context, context.count, message, message.count)
    }
  }
}
