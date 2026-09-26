// @pxl/native/modules/pxl-surface/ios/PxlKernelModule.swift

import ExpoModulesCore
import PxlNative

/// The kernel as the shell reaches it on a device (apps/native/src/platform/
/// core.ts): its memory, handed over as one ArrayBuffer over the region the
/// core took — no copy, so a ring the shell writes is the ring the kernel
/// reads — its doorbell, its device and the images it uploads; the surfaces
/// it draws into are the views' own (PxlSurfaceView.swift). The shell calls
/// them on the JavaScript thread, one at a time; the core runs each on its own
/// thread while this one waits (crates/pxl/web/native/src/thread.rs), and the
/// images' decode off both.
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

    /// The device's uniform offset alignment and whether it times its
    /// passes, or nil where the system has no GPU.
    Function("gpuOpen") { () -> [UInt32]? in
      var align: UInt32 = 0
      var timestamps: UInt32 = 0
      return pxl_gpu_open(&align, &timestamps) == 0 ? [align, timestamps] : nil
    }

    /// Decodes a PNG or a WebP and holds it for the renderer: its handle, or 0
    /// when it does not decode. Async, so it runs on the module's queue rather
    /// than the JavaScript thread: a large image costs no turn.
    AsyncFunction("imageHold") { (file: Data) -> UInt32 in
      file.withUnsafeBytes { bytes in
        pxl_image_hold(bytes.bindMemory(to: UInt8.self).baseAddress, UInt32(bytes.count))
      }
    }
  }
}

internal final class MemoryException: Exception, @unchecked Sendable {
  override var reason: String {
    "the core has no memory"
  }
}
