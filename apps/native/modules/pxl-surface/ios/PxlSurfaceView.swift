// @pxl/native/modules/pxl-surface/ios/PxlSurfaceView.swift

import ExpoModulesCore
import PxlNative
import QuartzCore
import UIKit

/// A view backed by a Metal layer, which the kernel draws into
/// (crates/pxl/web/native). The view only leaves its layer for the kernel
/// while it is in a window, and says so: a view reaches its window after it
/// is mounted and laid out, so the shell binds the kernel's surface to the
/// layer on `onAttach` and unbinds it on `onDetach`. The kernel sizes it and
/// draws every frame inside a turn, on the shell's thread, so nothing here
/// draws or runs a frame.
class PxlSurfaceView: ExpoView {
  override class var layerClass: AnyClass { CAMetalLayer.self }

  let onAttach = EventDispatcher()
  let onDetach = EventDispatcher()

  override func didMoveToWindow() {
    super.didMoveToWindow()
    if let window {
      layer.contentsScale = window.screen.scale
      pxl_surface_attach(Unmanaged.passUnretained(layer).toOpaque())
      onAttach()
    } else {
      pxl_surface_detach()
      onDetach()
    }
  }
}
