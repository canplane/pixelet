// @pxl/native/modules/platform/ios/BarBlur.swift

import ExpoModulesCore
import UIKit

/// The system's blur a bar stands on, for a backing whose material is not the
/// glass (apps/native/src/platform/backing.tsx): the thinnest of the system's
/// materials, whole from the start, since the shell's ground is laid over it
/// and a thicker one, the chrome iOS's own bars are drawn in, hid all that
/// scrolled under it. Not expo-blur's view, which
/// sets its effect in `draw(_:)` to stop an animation part way for its
/// intensity: iOS 26 never draws a visual effect view, so it blurs nothing.
class BarBlur: ExpoView {
  private let blur = UIVisualEffectView(effect: UIBlurEffect(style: .systemUltraThinMaterialLight))

  required init(appContext: AppContext? = nil) {
    super.init(appContext: appContext)
    blur.frame = bounds
    blur.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    blur.isUserInteractionEnabled = false
    addSubview(blur)
  }
}
