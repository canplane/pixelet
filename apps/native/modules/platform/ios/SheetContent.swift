// @pxl/native/modules/platform/ios/SheetContent.swift

import ExpoUI
import SwiftUI

/// SwiftUI's `presentationContentInteraction(.scrolls)` as a modifier the
/// shell can name (platform/panel.tsx): a drag on a sheet's content scrolls
/// the content, and only the sheet's top (its handle and header) resizes or
/// dismisses it. @expo/ui has no modifier for it, so it is registered with
/// @expo/ui's registry (PlatformModule's OnCreate).
struct ContentScrollsModifier: ViewModifier {
  func body(content: Content) -> some View {
    if #available(iOS 16.4, *) {
      content.presentationContentInteraction(.scrolls)
    } else {
      content
    }
  }
}

/// The modifier's name on the JavaScript side.
let contentScrollsModifier = "pxlContentScrolls"
