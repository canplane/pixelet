# The device's platform pieces

What the shell imports as `@platform/*` is the page's module on the web
(`apps/web/src/platform/web/`) and this directory's on a device: `backing.tsx` on
an iPhone, `backing.android.tsx` on Android, found by Metro's platform extensions.
`conform.ts` holds every one to the page's exports. This page is where each
piece's implementation is said, side by side, so where the platforms part
shows in one place, as the colors that part do in `theme.ts`.

## Which technology draws what

| Piece             | iPhone                                     | Android                | Page                  |
| ----------------- | ------------------------------------------ | ---------------------- | --------------------- |
| `panel`           | SwiftUI sheet (`@expo/ui` BottomSheet)     | the page's, + back     | React Native sheet    |
| `segmented`       | UIKit (`SegmentedControl.swift`)           | the page's             | React Native          |
| `toggle-row`      | UIKit switch (React Native's Switch)       | the page's             | the row is the switch |
| `backing`, `pill` | UIKit glass (expo-glass-effect)            | the page's fill        | fill                  |
| `backdrop`        | UIKit's blur (`Blur.swift`) under the fill | none                   | CSS backdrop blur     |
| `icon`            | SF Symbols (expo-symbols)                  | the page's lucide      | lucide                |
| `palette`         | system colors                              | the page's             | iOS's light values    |
| `above-keyboard`  | `KeyboardDock.swift`                       | `KeyboardDock.kt`      | visual viewport       |
| `surface`         | `SurfaceView.swift`                        | `KernelSurfaceView.kt` | canvas                |

Android takes the page's own drawing wherever Material's controls would read
as any Android app's; its backing is the page's fill, as the page's is.

## A control that takes a touch is UIKit's

On an iPhone, a control that takes a touch inside React Native's views is
UIKit's: React Native's own component, or a view of this module
(`modules/platform/ios/`). SwiftUI, through `@expo/ui`, draws what is
presented on its own (the sheet) and nothing a touch lands on inside React
Native.

A SwiftUI Toggle in the settings sheet sat in SwiftUI under React Native's
scroll view under SwiftUI again, and on a phone it took about three quick
taps in five; UIKit's switch in the same place took every one
(issue/20260928-a-control-in-two-layers.md). React Native's scroll view
never takes a touch from a UIKit control; it has no such promise for a
SwiftUI one.

`check:rules` holds `@expo/ui` to `panel.tsx`.
