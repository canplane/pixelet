// @pxl/native/modules/platform/ios/KeyboardDock.swift

import ExpoModulesCore
import UIKit

/// A view that stands on the keyboard (apps/native/src/platform/above-keyboard.tsx).
/// Its layout is the shell's, where it stands with no keyboard up; while one
/// is, the view is lifted by as much of it as covers the safe area, in the
/// animation the keyboard's own change runs in — its duration and its curve,
/// the keyboard's private one among them — so the view and the keys move as
/// one. Told from here rather than from the JavaScript, which hears of the
/// keyboard a thread hop later and would lay out after the keys had left.
///
/// The lift is a transform on the view, so the layout the shell gives it and
/// the lift the keyboard gives it are two things that never overwrite each
/// other. The view takes no touch of its own, only what stands on it.
class KeyboardDock: ExpoView {
  required init(appContext: AppContext? = nil) {
    super.init(appContext: appContext)
    NotificationCenter.default.addObserver(self, selector: #selector(keyboardWillChange(_:)), name: UIResponder.keyboardWillChangeFrameNotification, object: nil)
  }

  @objc private func keyboardWillChange(_ note: Notification) {
    guard let window, let info = note.userInfo,
          let end = (info[UIResponder.keyboardFrameEndUserInfoKey] as? NSValue)?.cgRectValue
    else { return }
    let covered = window.bounds.maxY - window.convert(end, from: window.screen.coordinateSpace).minY
    let lift = max(0, covered - window.safeAreaInsets.bottom)
    let duration = (info[UIResponder.keyboardAnimationDurationUserInfoKey] as? Double) ?? 0
    let curve = (info[UIResponder.keyboardAnimationCurveUserInfoKey] as? UInt) ?? UInt(UIView.AnimationCurve.easeInOut.rawValue)
    UIView.animate(withDuration: duration, delay: 0, options: [UIView.AnimationOptions(rawValue: curve << 16), .beginFromCurrentState]) {
      self.transform = CGAffineTransform(translationX: 0, y: -lift)
    }
  }

  override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
    let hit = super.hitTest(point, with: event)
    return hit === self ? nil : hit
  }
}
