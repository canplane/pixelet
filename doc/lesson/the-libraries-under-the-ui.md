# The libraries under the UI

**2026-11-17.** What react-native-gesture-handler 3.3, react-native-reanimated
4.7 and react-native-worklets 0.13 do to the code around them, learned in a
browser and on a Galaxy S23 while the UI's own gestures and motion were moved
onto them
([the UI's input and motion, left to libraries](../minutes/20261117-the-ui-s-input-and-motion-left-to-libraries.md)).
No check caught any of these: the types, the lint and the tests passed each
time, and each showed only when the thing was pressed.

## Keep clear of

- **A gesture's callback that reads or writes the file's own state.** The
  worklets Babel plugin turns every callback written inline in a gesture
  hook's config into a function made by a factory, which is handed the values
  around it as they stood when it was made. A number or a boolean goes in as a
  copy; a function or an object goes in by reference. `runOnJS` and
  `disableReanimated` change where the callback runs and not this. A flag one
  callback set was never seen by another: a tap did nothing and the drag was
  never told the core, from the commit that brought reanimated in until
  `51aa0285c2`. A callback passes its event's values to a function of the
  module, and the functions alone hold the state. An identifier given as a
  callback is followed and made a worklet too, so a callback is written as an
  arrow that calls the function.
- **A style sheet's `box-none` on an animated view.** Reanimated's view on the
  web copies each style object, which loses react-native-web's sheet and writes
  the style inline, where a `box-none` is drawn as `none`: every control under
  the layer stopped taking a press. What touches a view takes goes as the
  `pointerEvents` prop, which react-native-web draws through its own sheet.
- **Reanimated's `entering` and `exiting` on the web.** Once an entering is
  over, the library writes the view an absolute place and a size in pixels and
  leaves them. A keyframe's easing given as a Bézier plays linear. A view that
  is leaving is a clone, and showing it again starts a new view from the
  beginning.
- **A transition's end taken from its end event.** On the web the event is the
  browser's own, and none comes when the browser starts no transition, as when
  two poses land between two style recalcs. What waits on a motion waits its
  duration.
- **A dev server run with `CI=1`.** Metro then does not watch the tree. An
  edit after the first is not served, and what the browser shows is the old
  bundle, which reads as the edit having no effect.

## Check

A change to a gesture or a motion is pressed before it is called done: in a
browser for the web, and on a phone for a touch, `adb shell input` reaching a
single finger only. The served bundle is searched for the edit
(`curl … | grep`) before a result is believed.
