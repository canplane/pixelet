# The UI's input and motion, left to libraries

**2026-11-17** — recorded with the user, at the end of the session that did the
work. It began with a compass bar for the top strip (`95421da943`) and went on
to what that change kept running into: the UI's own state held in the core, and
whole mechanisms built by hand that a library already does. What the libraries
do to the code around them is
[the libraries under the UI](../lesson/the-libraries-under-the-ui.md).

## What was decided

- **The UI's context does not cross the WIT.** The user's. The gauges, the
  compass bar and the scale, show from the UI's own input alone: a turning drag
  or a pinch held, and a linger after it or after a wheel's zoom. The core no
  longer says that a zoom or a gesture is under way (`zooming`, `camera-busy`)
  and has no north-up command; each was the UI's input sent to the core and
  read back. A view that turns with the device shows no gauges.
- **Recognizing a gesture is the UI's.** The user's. The core had tracked
  pointers, told a tap from a drag and held the gestures' anchors since the
  kernel took the client's logic in September. That was where the code had
  been moved, and no one's decision; by the WIT's own rule a raw pointer does
  not cross where what is read is what it asks of the camera. The interface
  carries a pan, a turn, a tilt, a zoom, whether a hand drags the view, a tap
  and a hover (`6c1a04d0a0`). The core keeps the screen's work: the ground
  under a pan and the pick under a tap or a hover.
- **What a library does is not built here.** The user's, who had asked for none
  of what was removed and had not known the libraries existed. Gestures are
  react-native-gesture-handler's (`6c1a04d0a0`, `190e0d3a8e`), motion
  react-native-reanimated's (`d093697d5e`), the keyboard's height
  react-native-keyboard-controller's (`28882110e6`). Each is what a React
  Native app conventionally takes for it. What went: the core's pointer
  tracking and the gestures' anchors, the UI's map of pressed pointers and its
  release of them, the native views that forwarded touches, every PanResponder,
  a web engine and a native engine for motion with the spring solver under
  them, and the native views that lifted a layer with the keyboard.
- **A peer dependency is not a cost by itself.** The assistant had counted
  reanimated against the keyboard library that needs it, without counting what
  reanimated would replace. The user asked how that judgment had been made, and
  it had not been: the count was redone and came out the other way.
- **Pixelet's TypeScript follows TypeScript's general convention.** The user's,
  earlier in the session (`f0e4c12c90`, `2cf1ea745b`, `29f1af6f73`,
  `9eb1a8f0f6`): camelCase, the common short forms as they are, an acronym as a
  word (`Id`, never `ID`). xpute, the GPU plugin and generated code keep
  snake_case.

## What was tried and dropped

- **`Presence` and `Swap` on reanimated's `entering` and `exiting`.** Done,
  seen in a browser and taken back before it was committed. On the web the
  library pins a view that has entered to an absolute place and a width in
  pixels once the entering is over, so the address in the top strip stood
  frozen at the width it entered with; it plays a keyframe's easing as linear
  there; and an exit cannot be taken up again midway. The UI's own
  `presence.tsx` stays, and is not to be tried against that library again
  without those three answered.
- **Each `Motion` written out at its call site.** Not done. `Motion` is now a
  thin view over reanimated's CSS transitions, and writing it out would spread
  the same lines over some thirty places.
- **A sheet library for the web's and Android's sheet.** Looked at and left:
  it needs reanimated and serves the web poorly. An iPhone already takes the
  system's sheet.

## What the user approved as it stands

- A press held past half a second is no tap, as react-native-gesture-handler's
  tap has it. The core had put no limit on it.
- The toast's mark arrives on a curve that overshoots once in 400 ms, where a
  spring rang for 850 ms.
- react-native-keyboard-controller readies the keyboard once, unseen, as the
  app starts on iOS.
- `gesture` is out of the camera's order in `rule.rs`, its module being gone.

## What the user tried

What the session could not press itself, the user did, and found working:
two-finger gestures, a swatch's hold, the sheet's drag and the toasts' swipe on
a phone; a drag let go when the control centre or an alert takes the screen,
which rests on the library's own cancel; a tap and the layers moving with the
keys on an iPhone; and on the web a Shift drag, the message's send button and a
toast's mark changing.
