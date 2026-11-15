// @pxl/client/modules/platform/ios/WorldView.swift

import ExpoModulesCore
import UIKit
import PxlGpu

/// The world's surface, which hands its touches to the UI as pointer events
/// (`onPointer`), as a page's canvas gives its pointer events.
final class WorldView: SurfaceView {
  let attachEvent = EventDispatcher("onAttach")
  let detachEvent = EventDispatcher("onDetach")
  let pointerEvent = EventDispatcher("onPointer")

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

  private func tell(_ set: Set<UITouch>, _ phases: String...) {
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
        send(phase, id, Double(at.x), Double(at.y))
      }
      if phases.contains("up") || phases.contains("cancel") {
        ids[key] = nil
      }
    }
  }

  private func send(_ phase: String, _ id: Int32, _ x: Double, _ y: Double) {
    pointerEvent(["phase": phase, "id": id, "x": x, "y": y])
  }

  override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
    super.touchesBegan(touches, with: event)
    tell(touches, "down")
  }

  override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
    super.touchesMoved(touches, with: event)
    tell(touches, "move")
  }

  // A lifted finger also leaves the surface, as a page's pointerleave follows
  // its pointerup, so that nothing stays hovered under it.
  override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
    super.touchesEnded(touches, with: event)
    tell(touches, "up", "out")
  }

  override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
    super.touchesCancelled(touches, with: event)
    tell(touches, "cancel", "out")
  }

  override func didMoveToWindow() {
    super.didMoveToWindow()
    isMultipleTouchEnabled = true
    // A view gone from its window hears no more of its touches.
    if window == nil {
      ids.values.forEach { id in
        send("cancel", id, 0, 0)
        send("out", id, 0, 0)
      }
      ids.removeAll()
    }
  }
}
