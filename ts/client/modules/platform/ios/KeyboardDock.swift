// @pxl/native/modules/platform/ios/KeyboardDock.swift

import ExpoModulesCore
import UIKit

/// A view that stands on the keyboard (ts/client/src/ui/shared/component/above-keyboard.native.tsx).
/// The UI's layout places the view where it rests with no keyboard up, `rest`
/// off the bottom edge (ts/client/src/ui/shared/component/bottom-rest.ts). While a
/// keyboard is up, the view is lifted to stand `clearance` above it. The lift
/// runs in the animation of the keyboard's own change, with its duration and
/// its curve, which can be a private one, so the view and the keys move as one.
/// The lift is started here rather than from the JavaScript, which hears of the
/// keyboard a thread hop later and would lay out after the keys had left.
///
/// The view moves by a transform, which React Native never sees, because it
/// measures a view where its layout put it. On a transformed view, a press
/// whose finger slides by a point is taken for one that has left the view. So
/// the transform is only ever the part of the lift that the layout has not
/// taken yet. Once the keys have arrived, the view reports the lift
/// (`onLift`). The UI then lays the view out that much higher and reports it
/// back (`layoutLift`), and the transform falls to zero. At rest, the view is
/// where the layout says it is. The view takes no touch itself; only what
/// stands on it does.
class KeyboardDock: ExpoView {
  /// The gap the UI keeps from the bottom edge and from the keyboard. It is not
  /// named `inset`, because React Native reads that prop as a layout style.
  var clearance: CGFloat = 0
  /// How far off the bottom edge the UI rests the view. The lift is measured
  /// from there, so that the rule for where the view rests stays the UI's
  /// alone.
  var rest: CGFloat = 0
  /// How much of the lift the UI's layout has taken.
  var layoutLift: CGFloat = 0 {
    didSet { transform = CGAffineTransform(translationX: 0, y: -(lift - layoutLift)) }
  }
  let onLift = EventDispatcher()

  /// How much of the window the keyboard covers, as last reported.
  private var keyboard: CGFloat = 0
  private var lift: CGFloat { max(0, keyboard + clearance - rest) }

  required init(appContext: AppContext? = nil) {
    super.init(appContext: appContext)
    NotificationCenter.default.addObserver(self, selector: #selector(keyboardWillChange(_:)), name: UIResponder.keyboardWillChangeFrameNotification, object: nil)
  }

  @objc private func keyboardWillChange(_ note: Notification) {
    guard let window, let info = note.userInfo,
          let end = (info[UIResponder.keyboardFrameEndUserInfoKey] as? NSValue)?.cgRectValue
    else { return }
    keyboard = window.bounds.maxY - window.convert(end, from: window.screen.coordinateSpace).minY
    let target = lift
    let duration = (info[UIResponder.keyboardAnimationDurationUserInfoKey] as? Double) ?? 0
    let curve = (info[UIResponder.keyboardAnimationCurveUserInfoKey] as? UInt) ?? UInt(UIView.AnimationCurve.easeInOut.rawValue)
    UIView.animate(withDuration: duration, delay: 0, options: [UIView.AnimationOptions(rawValue: curve << 16), .beginFromCurrentState]) {
      self.transform = CGAffineTransform(translationX: 0, y: -(target - self.layoutLift))
    } completion: { finished in
      // An animation cut short was taken over by a later change, which reports
      // its own lift when it ends.
      if finished { self.onLift(["lift": target]) }
    }
  }

  override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
    let hit = super.hitTest(point, with: event)
    return hit === self ? nil : hit
  }
}
