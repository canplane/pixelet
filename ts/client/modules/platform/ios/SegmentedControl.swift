// @pxl/client/modules/platform/ios/SegmentedControl.swift

import ExpoModulesCore
import UIKit

/// UIKit's segmented control, for the UI's Segmented
/// (ts/client/src/ui/shared/component/segmented.ios.tsx). It is UIKit's rather than
/// SwiftUI's Picker, because a control that takes a touch inside React
/// Native's views must be UIKit's (ts/client/src/ui/shared/component/README.md says
/// why).
///
/// The UI's value rules. The control shows a pick at once and reports it, and
/// the UI's `selectedIndex` then sets what stands.
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
