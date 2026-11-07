// @pxl/native/modules/platform/ios/SegmentedControl.swift

import ExpoModulesCore
import UIKit

/// UIKit's segmented control, for the UI's Segmented
/// (ts/client/src/ui/component/segmented.ios.tsx). UIKit's rather than SwiftUI's
/// Picker: a control that takes a touch inside React Native's views is
/// UIKit's (ts/client/src/ui/component/README.md says why).
///
/// Held to the UI's value: a pick is reported and the control shows it at
/// once, and the UI's `selectedIndex` then says what stands.
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
    control.addTarget(self, action: #selector(pick), for: .valueChanged)
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    control.frame = bounds
  }

  @objc private func pick() {
    onPick(["index": control.selectedSegmentIndex])
  }
}
