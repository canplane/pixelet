// @pxl/native/modules/pxl-surface/ios/PxlSurfaceView.swift

import ExpoModulesCore
import PxlNative
import QuartzCore
import UIKit

/// A view backed by a Metal layer, which the kernel makes its surface on
/// (crates/pxl/web/native). The surface opens once the view has a window and
/// a size, and closes when it leaves the window, so the layer always outlives
/// it.
class PxlSurfaceView: ExpoView {
  override class var layerClass: AnyClass { CAMetalLayer.self }

  private var link: CADisplayLink?

  private var pixels: (UInt32, UInt32) {
    let scale = window?.screen.scale ?? traitCollection.displayScale
    return (UInt32(bounds.width * scale), UInt32(bounds.height * scale))
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    guard window != nil else { return }
    layer.contentsScale = window?.screen.scale ?? traitCollection.displayScale
    let (width, height) = pixels
    guard width > 0, height > 0 else { return }
    if link == nil {
      let opened = pxl_surface_open(Unmanaged.passUnretained(layer).toOpaque(), width, height)
      guard opened == 0 else { return }
      let link = CADisplayLink(target: self, selector: #selector(tick))
      link.add(to: .main, forMode: .common)
      self.link = link
    } else {
      pxl_surface_resize(width, height)
    }
  }

  override func didMoveToWindow() {
    super.didMoveToWindow()
    if window == nil { close() } else { setNeedsLayout() }
  }

  @objc private func tick() {
    pxl_surface_draw()
  }

  private func close() {
    guard let link else { return }
    link.invalidate()
    self.link = nil
    pxl_surface_close()
  }
}
