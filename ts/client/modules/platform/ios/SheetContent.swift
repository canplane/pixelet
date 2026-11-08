// @pxl/native/modules/platform/ios/SheetContent.swift

import ExpoUI
import SwiftUI

/// SwiftUI's `presentationContentInteraction(.scrolls)`, as a modifier the UI
/// can name (ts/client/src/ui/component/panel.ios.tsx). With it, a drag on a
/// sheet's content scrolls the content, and only the sheet's top, its handle
/// and its header, resizes or dismisses the sheet. @expo/ui has no modifier
/// for it, so PlatformModule's OnCreate registers this one with @expo/ui's
/// registry.
struct ContentScrollsModifier: ViewModifier {
  func body(content: Content) -> some View {
    content.presentationContentInteraction(.scrolls)
  }
}

/// The modifier's name on the JavaScript side.
let contentScrollsModifier = "pxlContentScrolls"
