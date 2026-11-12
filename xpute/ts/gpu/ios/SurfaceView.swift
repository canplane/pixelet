// @xpute/gpu/ios/SurfaceView.swift

import ExpoModulesCore
import CXputeGpu
import QuartzCore
import UIKit

/// A core's surface draws into this view, which a Metal layer backs. The view
/// owns its layer, so it binds the core's surface to the layer itself:
/// it binds the surface the app names (`surface`) while the view is in a
/// window, and never past the moment it lets the layer go. A view reaches
/// its window only after it is mounted and laid out, so it tells when the
/// surface is bound (`onAttach`) and when it no longer is (`onDetach`). The
/// core sizes the surface and draws every frame inside an entry, so nothing
/// here draws, and the view takes no input: a program that wants the view's
/// touches subclasses it.
open class SurfaceView: ExpoView {
  override open class var layerClass: AnyClass { CAMetalLayer.self }

  let onAttach = EventDispatcher()
  let onDetach = EventDispatcher()

  /// The core's surface the app wants shown here; 0 for none.
  public var surface: UInt32 = 0 {
    didSet { rebind() }
  }

  /// The surface the layer is bound to; 0 for none.
  private var bound: UInt32 = 0

  /// The view tells of its surface through these. Expo finds a view's event
  /// dispatchers among the stored properties of the view's own class, so a
  /// subclass declares its own and overrides these two.
  open func attached(_ surface: UInt32) {
    onAttach(["surface": surface])
  }

  open func detached() {
    onDetach()
  }

  override open func didMoveToWindow() {
    super.didMoveToWindow()
    rebind()
  }

  private func rebind() {
    let wanted = window == nil ? 0 : surface
    if bound == wanted { return }
    if bound != 0 {
      xpute_gpu_surface_unbind(bound)
      bound = 0
      detached()
    }
    guard wanted != 0, let window else { return }
    layer.contentsScale = window.screen.scale
    if xpute_gpu_surface_bind(wanted, Unmanaged.passUnretained(layer).toOpaque()) == 0 {
      bound = wanted
      attached(wanted)
    }
  }
}
