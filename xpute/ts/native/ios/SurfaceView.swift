// @xpute/native/ios/SurfaceView.swift

import ExpoModulesCore
import CXpute
import QuartzCore
import UIKit

/// A core's surface draws into this view, which a Metal layer backs. The view
/// owns its layer, so it binds the core's surface to the layer itself:
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

  /// Whether this view tells the core its touches (`xpute_touch`).
  var touches = false {
    didSet { isMultipleTouchEnabled = touches }
  }

  /// The surface the layer is bound to; 0 for none.
  private var bound: UInt32 = 0

  /// The ids of the touches on the view; a touch keeps its object from down to
  /// up. A touch takes the lowest free id, as Android's pointers do, so that a
  /// touch whose end never came is replaced by the next.
  private var ids: [ObjectIdentifier: Int32] = [:]

  private func tell(_ set: Set<UITouch>, _ phases: UInt32...) {
    guard touches else { return }
    for touch in set {
      let key = ObjectIdentifier(touch)
      let id: Int32
      if let known = ids[key] {
        id = known
      } else {
        id = (0...).first { !ids.values.contains(Int32($0)) }.map { Int32($0) } ?? 0
        ids[key] = id
      }
      let at = touch.location(in: self)
      for phase in phases {
        xpute_touch(phase, id, Double(at.x), Double(at.y))
      }
      if phases.contains(2) || phases.contains(4) {
        ids[key] = nil
      }
    }
  }

  override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
    super.touchesBegan(touches, with: event)
    tell(touches, 0)
  }

  override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
    super.touchesMoved(touches, with: event)
    tell(touches, 1)
  }

  // A lifted finger also leaves the surface, as a page's pointerleave follows
  // its pointerup, so that nothing stays hovered under it.
  override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
    super.touchesEnded(touches, with: event)
    tell(touches, 2, 3)
  }

  override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
    super.touchesCancelled(touches, with: event)
    tell(touches, 4, 3)
  }

  override func didMoveToWindow() {
    super.didMoveToWindow()
    rebind()
    // A view gone from its window hears no more of its touches.
    if window == nil {
      ids.values.forEach { id in
        xpute_touch(4, id, 0, 0)
        xpute_touch(3, id, 0, 0)
      }
      ids.removeAll()
    }
  }

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
