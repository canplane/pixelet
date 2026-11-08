// @pxl/native/modules/platform/ios/Blur.swift

import ExpoModulesCore
import UIKit

/// The system blur that a see-through backing stands on
/// (ts/client/src/ui/component/backing.ios.tsx). It is the thinnest of the
/// system's materials, at full strength from the start. The UI lays its own
/// fill over it, and a thicker material, such as the chrome iOS draws its own
/// bars in, hides everything that passes under it. expo-blur's view is not
/// used: it sets its effect in `draw(_:)`, to stop an animation part way for
/// its intensity, and iOS 26 never calls `draw(_:)` on a visual effect view,
/// so that view blurs nothing.
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
