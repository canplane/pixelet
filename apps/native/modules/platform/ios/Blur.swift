// @pxl/native/modules/platform/ios/Blur.swift

import ExpoModulesCore
import UIKit

/// The system's blur a see-through backing stands on
/// (apps/client/src/shell/ui/backing.ios.tsx): the thinnest of the system's
/// materials, whole from the start, since the shell's fill is laid over it
/// and a thicker one, the chrome iOS's own bars are drawn in, hid all that
/// passed under it. Not expo-blur's view, which
/// sets its effect in `draw(_:)` to stop an animation part way for its
/// intensity: iOS 26 never draws a visual effect view, so it blurs nothing.
class Blur: ExpoView {
  private let blur = UIVisualEffectView(effect: UIBlurEffect(style: .systemUltraThinMaterialLight))

  required init(appContext: AppContext? = nil) {
    super.init(appContext: appContext)
    blur.frame = bounds
    blur.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    blur.isUserInteractionEnabled = false
    addSubview(blur)
  }
}
