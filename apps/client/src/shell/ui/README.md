# Where the platforms part in the shell's pieces

A piece the platforms draw differently is one file and its overrides beside
it, as React Native names them: `pill.tsx` is the page's and Android's,
`pill.ios.tsx` an iPhone's; `motion/motion.native.tsx` both devices'. Metro
takes the file of the device's platform, Vite the default, and every import
names the default. An override re-exports the default and replaces only what it
draws itself, and `apps/client/src/conform.ts` holds each to its default's
exports. What only the place gives — the kernel's memory, the GPU, storage,
text, the network, the sensors, the surfaces — is not here but the app's
bracket (`apps/client/src/bracket.tsx`): the world's surface, for one, is a
canvas on the page (`apps/web/src/surface.tsx`) and a native view on a
device (`apps/native/src/surface.tsx`). This page says each piece's
implementation side by side, so where the platforms part shows in one
place, as the colors that part do in `theme.ts`.

## Which technology draws what

| Piece            | iPhone                                                                            | Android             | Page                                                |
| ---------------- | --------------------------------------------------------------------------------- | ------------------- | --------------------------------------------------- |
| `panel`          | SwiftUI sheet (`@expo/ui` BottomSheet)                                            | the page's, + back  | React Native sheet                                  |
| `segmented`      | UIKit (`SegmentedControl.swift`)                                                  | the page's          | React Native                                        |
| `toggle-row`     | UIKit switch (React Native's Switch)                                              | the page's          | the row is the switch                               |
| `backing`        | Liquid Glass (expo-glass-effect), else UIKit's blur (`Blur.swift`) under the fill | the fill, solid     | the fill, CSS backdrop blur under a see-through one |
| `pill`           | UIKit glass (expo-glass-effect)                                                   | the page's fill     | fill                                                |
| `text-field`     | system clear button                                                               | the page's          | its own clear button                                |
| `icon`           | SF Symbols (expo-symbols)                                                         | the page's Phosphor | Phosphor                                            |
| `palette`        | system colors                                                                     | the page's          | iOS's light values                                  |
| `above-keyboard` | `KeyboardDock.swift`                                                              | `KeyboardDock.kt`   | visual viewport                                     |

Android takes the page's own drawing wherever Material's controls would read
as any Android app's. Its backing is the fill made solid: it cannot blur the
map, which is a native surface, and a see-through fill unblurred is
unreadable.

A device draws text at the size the phone's Text Size setting asks for, and
a page ignores it, so a view that fits on a page can overflow on a device.
The iOS simulator takes the setting from the command line: `xcrun simctl ui
booted content_size extra-small`, then `accessibility-extra-extra-extra-large`,
then `large` again.

## A control that takes a touch is UIKit's

On an iPhone, a control that takes a touch inside React Native's views is
UIKit's: React Native's own component, or a view of this module
(`modules/platform/ios/`). SwiftUI, through `@expo/ui`, draws what is
presented on its own (the sheet) and nothing a touch lands on inside React
Native.

A SwiftUI Toggle in the settings sheet sat in SwiftUI under React Native's
scroll view under SwiftUI again, and on a phone it took about three quick
taps in five; UIKit's switch in the same place took every one
(doc/minutes/archive/20260928-a-control-in-two-layers.md). React Native's scroll view
never takes a touch from a UIKit control; it has no such promise for a
SwiftUI one.

`check:rules` holds `@expo/ui` to `panel.ios.tsx`.
