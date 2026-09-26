// @pxl/native/modules/pxl-surface/ios/PxlKernelModule.swift

import ExpoModulesCore
import PxlNative

/// The kernel as the shell reaches it on a device (apps/native/src/platform/
/// core.ts): its memory, handed over as one ArrayBuffer over the region the
/// core took — no copy, so a ring the shell writes is the ring the kernel
/// reads — and its doorbell. Both run on the JavaScript thread, where the
/// shell calls them, which makes it the kernel's one thread.
public class PxlKernelModule: Module {
  public func definition() -> ModuleDefinition {
    Name("PxlKernel")

    Function("memory") { () throws -> NativeArrayBuffer in
      var bytes: UInt32 = 0
      guard let at = pxl_memory(&bytes) else { throw MemoryException() }
      // The region is the core's for the app's life: nothing is let go when
      // the buffer is.
      return try NativeArrayBuffer.wrap(dataWithoutCopy: UnsafeMutableRawBufferPointer(start: at, count: Int(bytes)), cleanup: {})
    }

    Function("interrupt") { (quotaMs: Double) -> Double in
      pxl_interrupt(quotaMs)
    }
  }
}

internal final class MemoryException: Exception, @unchecked Sendable {
  override var reason: String {
    "the core has no memory"
  }
}
