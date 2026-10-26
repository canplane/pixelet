# A control in two layers

On an iPhone the settings sheet's switches took about three quick taps in five.
A tap held a little longer always took, so it read as a switch that was slow to
answer rather than one that did not work, and it was noticed only after using
the settings for a while. The page and Android, whose rows are React Native,
took every tap.

## What the switch sat in

The sheet is SwiftUI's (`@expo/ui` BottomSheet), its content React Native
(RNHostView), the content a React Native scroll view, and each switch was
SwiftUI's Toggle in a Host of its own:

```
SwiftUI sheet → React Native → React Native's scroll view (UIKit) → SwiftUI Toggle
```

Three touch systems stand between a finger and the Toggle: SwiftUI's gestures,
UIKit's scroll view, and React Native's touch handler.

## How it was narrowed

|                                                              |                                                                                                          |
| ------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| React Native's scroll view delays touches to its content     | It does not: React Native sets `delaysContentTouches = NO` (RCTScrollViewComponentView.mm).              |
| React Native's touch handler cancels the Toggle's            | It does not: `cancelsTouchesInView = NO`, and it lets other recognizers win (RCTSurfaceTouchHandler.mm). |
| The sheet's `presentationContentInteraction(.scrolls)`       | Removed on the simulator; nothing changed.                                                               |
| The sheet, and the RNHostView under it                       | A Toggle on the map, outside any sheet, behaved the same on the simulator.                               |
| The row is the switch, the Toggle only shown                 | On the phone, every tap took. The Toggle's press, its thumb swelling into glass, went with it.           |
| UIKit's switch (React Native's Switch), taking its own touch | On the phone, every tap took, and the thumb swells under a finger.                                       |

The simulator misled twice. A mouse click there presses and lets go in the same
instant, and every system switch ignored it — SwiftUI's and UIKit's, in the
sheet and out of it — while a press of 50 ms took. That is what made the sheet
look guilty and then innocent; neither was measured by it. Only the phone,
tapped by a finger, told the two switches apart.

## What was decided

A control that takes a touch inside React Native's views is UIKit's: React
Native's own component, or a view of the device app's module. SwiftUI draws what
is presented on its own, the sheet, and nothing a touch lands on inside React
Native. React Native's scroll view never takes a touch from a UIKit control; it
makes no such promise for a SwiftUI one, and one was seen to lose it.

- The switch is React Native's Switch, taking its own touch
  (apps/client/src/shell/ui/toggle-row.ios.tsx).
- The segmented control, the other SwiftUI control inside the sheet, is UIKit's,
  a view of the module (SegmentedControl.swift).
- `check:rules` holds `@expo/ui` to the sheet (panel.tsx), and
  apps/client/src/shell/ui/README.md says which technology draws each piece.

## Not done

The exact mechanism by which the Toggle lost a tap was not found; it lies in how
SwiftUI decides a tap under a UIKit scroll view, which is not ours to change.
Moving the sheet to React Native as well — one touch system in all — was weighed
and left: it would lose the system sheet's own motion and glass, and with the
controls UIKit's nothing in the sheet was seen to fail.
