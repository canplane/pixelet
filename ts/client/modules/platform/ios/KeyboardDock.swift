// @pxl/native/modules/platform/ios/KeyboardDock.swift

import ExpoModulesCore
import UIKit

/// A view that stands on the keyboard (ts/client/src/shell/ui/above-keyboard.native.tsx).
/// Its layout is the shell's, where it rests with no keyboard up, `rest` off
/// the bottom edge (ts/client/src/shell/ui/bottom-rest.ts). While a keyboard is up, the
/// view is lifted to stand `clearance` above it, in the animation the
/// keyboard's own change runs in — its duration and its curve,
/// the keyboard's private one among them — so the view and the keys move as
/// one. Told from here rather than from the JavaScript, which hears of the
/// keyboard a thread hop later and would lay out after the keys had left.
///
/// The moving is a transform, which React Native never sees: it measures a
/// view where its layout put it, and a press that slid a point on a view
/// lifted where no measure looks is taken for one that had left it. So the
/// transform is only ever the part of the lift the layout has not taken yet.
/// Once the keys have arrived, the lift is said (`onLift`), the shell lays
/// the view out that much higher and says so (`applied`), and the transform
/// is none: at rest, the view is where the layout says it is. The view takes
/// no touch of its own, only what stands on it.
class KeyboardDock: ExpoView {
  /// The gap the shell keeps from the bottom edge and from the keyboard.
  /// Not `inset`, which React Native reads as a layout style.
  var clearance: CGFloat = 0
  /// How far off the bottom edge the shell rests the view; the lift is
  /// measured from there, so the rule of where it rests is the shell's alone.
  var rest: CGFloat = 0
  /// How much of the lift the shell's layout carries.
  var applied: CGFloat = 0 {
    didSet { transform = CGAffineTransform(translationX: 0, y: -(lift - applied)) }
  }
  let onLift = EventDispatcher()

  /// How much of the window the keyboard covers, as it was last told.
  private var covered: CGFloat = 0
  private var lift: CGFloat { max(0, covered + clearance - rest) }

  required init(appContext: AppContext? = nil) {
    super.init(appContext: appContext)
    NotificationCenter.default.addObserver(self, selector: #selector(keyboardWillChange(_:)), name: UIResponder.keyboardWillChangeFrameNotification, object: nil)
  }

  @objc private func keyboardWillChange(_ note: Notification) {
    guard let window, let info = note.userInfo,
          let end = (info[UIResponder.keyboardFrameEndUserInfoKey] as? NSValue)?.cgRectValue
    else { return }
    covered = window.bounds.maxY - window.convert(end, from: window.screen.coordinateSpace).minY
    let target = lift
    let duration = (info[UIResponder.keyboardAnimationDurationUserInfoKey] as? Double) ?? 0
    let curve = (info[UIResponder.keyboardAnimationCurveUserInfoKey] as? UInt) ?? UInt(UIView.AnimationCurve.easeInOut.rawValue)
    UIView.animate(withDuration: duration, delay: 0, options: [UIView.AnimationOptions(rawValue: curve << 16), .beginFromCurrentState]) {
      self.transform = CGAffineTransform(translationX: 0, y: -(target - self.applied))
    } completion: { finished in
      // A later change that took over says its own lift when it ends.
      if finished { self.onLift(["lift": target]) }
    }
  }

  override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
    let hit = super.hitTest(point, with: event)
    return hit === self ? nil : hit
  }
}
