# Where the platforms part in the shell's pieces

A piece the platforms draw differently is one file and its overrides beside
it, as React Native names them: `pill.tsx` is the page's and Android's,
`pill.ios.tsx` an iPhone's; `motion.native.tsx` both devices'. Metro takes
the file of the device's platform, Vite the default, and every import names
the default. An override re-exports the default and replaces only what it
draws itself, and `apps/client/src/conform.ts` holds each to its default's
exports. What only the place gives — the kernel's memory, the GPU, storage,
text, the network, the sensors, the surfaces — is not here but the app's
bracket (`apps/client/src/bracket.tsx`). This page says each piece's
implementation side by side, so where the platforms part shows in one
place, as the colors that part do in `theme.ts`.

## Which technology draws what

| Piece             | iPhone                                     | Android                | Page                  |
| ----------------- | ------------------------------------------ | ---------------------- | --------------------- |
| `panel`           | SwiftUI sheet (`@expo/ui` BottomSheet)     | the page's, + back     | React Native sheet    |
| `segmented`       | UIKit (`SegmentedControl.swift`)           | the page's             | React Native          |
| `toggle-row`      | UIKit switch (React Native's Switch)       | the page's             | the row is the switch |
| `backing`, `pill` | UIKit glass (expo-glass-effect)            | the page's fill        | fill                  |
| `text-field`      | system clear button                        | the page's             | its own clear button  |
| `backdrop`        | UIKit's blur (`Blur.swift`) under the fill | none                   | CSS backdrop blur     |
| `icon`            | SF Symbols (expo-symbols)                  | the page's Phosphor    | Phosphor              |
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
(doc/minutes/20260928-a-control-in-two-layers.md). React Native's scroll view
never takes a touch from a UIKit control; it has no such promise for a
SwiftUI one.

`check:rules` holds `@expo/ui` to `panel.tsx`.
