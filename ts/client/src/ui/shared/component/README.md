# Where the platforms part in the UI's pieces

A piece the platforms draw differently is a pair of files, or one file and its
overrides beside it, as React Native names them: `backdrop.web.ts` is the web's
and `backdrop.native.ts` both iOS's and Android's; `segmented.tsx` is the web's
and Android's, `segmented.ios.tsx` an iPhone's. Metro takes the file of the
platform it builds for (`ts/build/path.ts` `platformFile`), and every import
names the plain file (`backdrop.ts`, `segmented.tsx`). An override replaces what
it draws itself and re-exports the rest of its default, and
`ts/client/src/conform.ts` holds each native file to the web file's exports and
each override to its default's.

What only the place gives — storage, the network, the sensors, the surfaces — is
not here but in the client's services (`ts/client/src/service/platform/`), a web
file beside a native one: the world's surface, for one, is a canvas on the web
(`service/platform/surface.web.tsx`) and a native view on iOS and Android
(`service/platform/surface.native.tsx`). The table says the pieces'
implementations side by side, so where the platforms part shows in one place, as
the colors that part do in `theme.ts`.

## Which technology draws what

| Piece            | iPhone                                                                                                                   | Android            | Web                                                 |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------ | ------------------ | --------------------------------------------------- |
| `panel`          | SwiftUI sheet (`@expo/ui` BottomSheet)                                                                                   | the web's, + back  | React Native sheet                                  |
| `segmented`      | UIKit (`SegmentedControl.swift`)                                                                                         | the web's          | React Native                                        |
| `toggle-row`     | UIKit switch (React Native's Switch)                                                                                     | the web's          | the row is the switch                               |
| `backing`        | UIKit's blur (`Blur.swift`) under the fill for a material; else Liquid Glass (expo-glass-effect), or the fill without it | the fill, solid    | the fill, CSS backdrop blur under a see-through one |
| `pill`           | UIKit glass (expo-glass-effect)                                                                                          | the web's fill     | fill                                                |
| `text-field`     | system clear button                                                                                                      | the web's          | its own clear button                                |
| `icon`           | SF Symbols (expo-symbols)                                                                                                | the web's Phosphor | Phosphor                                            |
| `palette`        | system colors                                                                                                            | the web's          | iOS's light values                                  |
| `above-keyboard` | react-native-keyboard-controller                                                                                         | the iPhone's       | visual viewport                                     |

Android takes the web's own drawing wherever Material's controls would read as
any Android app's. Its backing is the fill made solid: it cannot blur the map,
which is a native surface, and a see-through fill unblurred is unreadable.

A native app draws text at the size the phone's Text Size setting asks for, and
the web ignores it, so a view that fits on the web can overflow natively.

## A control that takes a touch is UIKit's

On an iPhone, a control that takes a touch inside React Native's views is
UIKit's: React Native's own component, or a view of the platform module
(`ts/client/modules/platform/ios/`). SwiftUI, through `@expo/ui`, draws what
is presented on its own (the sheet) and nothing a touch lands on inside React
Native.

React Native's scroll view never takes a touch from a UIKit control; it has no
such promise for a SwiftUI one.

`@expo/ui` is imported by `panel.ios.tsx` alone.
