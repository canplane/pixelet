// @xpute/native/ios/SurfaceView.swift

import ExpoModulesCore
import CXpute
import QuartzCore
import UIKit

/// A view backed by a Metal layer, which a core's surface draws into. The
/// view owns its layer, so it binds the core's surface to the layer itself:
/// it binds the surface the app names (`surface`) while the view is in a
/// window, and never past the moment it lets the layer go. A view reaches
/// its window only after it is mounted and laid out, so it tells when the
/// surface is bound (`onAttach`) and when it no longer is (`onDetach`). The
/// core sizes the surface and draws every frame inside a turn, so nothing
/// here draws.
class SurfaceView: ExpoView {
  override class var layerClass: AnyClass { CAMetalLayer.self }

  let onAttach = EventDispatcher()
  let onDetach = EventDispatcher()

  /// The core's surface the app wants shown here; 0 for none.
  var surface: UInt32 = 0 {
    didSet { rebind() }
  }

  /// The surface the layer is bound to; 0 for none.
  private var bound: UInt32 = 0

  override func didMoveToWindow() {
    super.didMoveToWindow()
    rebind()
  }

  /// Binds the layer to the surface wanted now, and lets go of one no longer
  /// wanted.
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
