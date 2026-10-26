// @pxl/native/modules/platform/ios/SurfaceView.swift

import ExpoModulesCore
import PxlNative
import QuartzCore
import UIKit

/// A view backed by a Metal layer, which the kernel draws into
/// (rust/client-bracket/native). The view owns its layer, so it binds the kernel's
/// surface to it itself: the surface the shell names (`surface`), while the
/// view is in a window, and never past the moment it lets the layer go. A
/// view reaches its window after it is mounted and laid out, so it says when
/// it is bound (`onAttach`) and when it is not (`onDetach`). The kernel sizes
/// the surface and draws every frame inside a turn, so nothing here draws or
/// runs a frame.
class SurfaceView: ExpoView {
  override class var layerClass: AnyClass { CAMetalLayer.self }

  let onAttach = EventDispatcher()
  let onDetach = EventDispatcher()

  /// The kernel's surface the shell wants shown here; 0 for none.
  var surface: UInt32 = 0 {
    didSet { rebind() }
  }

  /// The surface the layer is bound to; 0 for none.
  private var bound: UInt32 = 0

  override func didMoveToWindow() {
    super.didMoveToWindow()
    rebind()
  }

  /// Binds the layer to what is wanted, letting go of what is not.
  private func rebind() {
    let wanted = window == nil ? 0 : surface
    if bound == wanted { return }
    if bound != 0 {
      xpute_gpu_surface_unbind(bound)
      bound = 0
      onDetach()
    }
    guard wanted != 0, let window else { return }
    layer.contentsScale = window.screen.scale
    if xpute_gpu_surface_bind(wanted, Unmanaged.passUnretained(layer).toOpaque()) == 0 {
      bound = wanted
      onAttach(["surface": wanted])
    }
  }
}
