// @pxl/native/modules/platform/ios/SegmentedControl.swift

import ExpoModulesCore
import UIKit

/// UIKit's segmented control, for the shell's Segmented
/// (apps/native/src/platform/segmented.tsx). UIKit's rather than SwiftUI's
/// Picker: a control that takes a touch inside React Native's views is
/// UIKit's (apps/native/src/platform/README.md says why).
///
/// Held to the shell's value: a pick is reported and the control shows it at
/// once, and the shell's `selectedIndex` then says what stands.
class SegmentedControl: ExpoView {
  private let control = UISegmentedControl()
  let onPick = EventDispatcher()

  var labels: [String] = [] {
    didSet {
      guard labels != oldValue else { return }
      control.removeAllSegments()
      for (i, label) in labels.enumerated() {
        control.insertSegment(withTitle: label, at: i, animated: false)
      }
      control.selectedSegmentIndex = selectedIndex
    }
  }

  var selectedIndex: Int = 0 {
    didSet { control.selectedSegmentIndex = selectedIndex }
  }

  required init(appContext: AppContext? = nil) {
    super.init(appContext: appContext)
    addSubview(control)
    control.addTarget(self, action: #selector(picked), for: .valueChanged)
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    control.frame = bounds
  }

  @objc private func picked() {
    onPick(["index": control.selectedSegmentIndex])
  }
}
