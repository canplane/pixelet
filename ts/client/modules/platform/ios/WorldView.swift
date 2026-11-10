// @pxl/native/modules/platform/ios/WorldView.swift

import ExpoModulesCore
import PxlNative
import UIKit
import XputeGpu

/// The world's surface, which tells the core its touches (`pxl_touch`).
final class WorldView: SurfaceView {
  let attachEvent = EventDispatcher("onAttach")
  let detachEvent = EventDispatcher("onDetach")

  override func attached(_ surface: UInt32) {
    attachEvent(["surface": surface])
  }

  override func detached() {
    detachEvent()
  }

  /// The ids of the touches on the view; a touch keeps its object from down to
  /// up. A touch takes the lowest free id, as Android's pointers do, so that a
  /// touch whose end never came is replaced by the next.
  private var ids: [ObjectIdentifier: Int32] = [:]

  private func tell(_ set: Set<UITouch>, _ phases: UInt32...) {
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
        pxl_touch(phase, id, Double(at.x), Double(at.y))
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
    isMultipleTouchEnabled = true
    // A view gone from its window hears no more of its touches.
    if window == nil {
      ids.values.forEach { id in
        pxl_touch(4, id, 0, 0)
        pxl_touch(3, id, 0, 0)
      }
      ids.removeAll()
    }
  }
}
