# Three sizes of a control

Every control the shell draws has three sizes, each for a different thing,
and they were being chosen together — one number for a button, a glyph
worked out from it — which gave the strip's slots a target too small to hit
and a glyph that shifted with every size of button. They are chosen apart:

| Size                   | For           | Set by                                          | Here                                         |
| ---------------------- | ------------- | ----------------------------------------------- | -------------------------------------------- |
| where a press lands    | the finger    | the platform: 44 on iOS (HIG), 48 dp on Android | at least 44, everywhere                      |
| the container one sees | the hierarchy | where the control stands                        | 48 over the map, 44 on a sheet or in a strip |
| the glyph in it        | the eye       | reading, the same whatever holds it             | 20                                           |

A press may land wider than the container (a sheet's control stood 40 with a
44 target through `hitSlop` for a while), never narrower. The container says
how loud a control is: a control floating over the map, whose ground is busy
and moving, is a step larger than one set in a sheet or in a row of tools. The
glyph does not follow the container: a glyph worked out as a share of it
differed by a few tenths between a 48 and a 44 standing side by side in the
same strip.

## What they were measured against

On the iOS 26 simulator, Apple Maps; and ChatGPT on an iPhone, from screen
captures, at 1.5 pixels a point.

|                                     | Container | Glyph    | Glyph / container |
| ----------------------------------- | --------- | -------- | ----------------- |
| Maps, the map's controls            | 48        | 17–18    | 0.37              |
| Maps, a sheet's close and field     | 44        | about 17 | 0.38              |
| ChatGPT, the top row's round ones   | 48        | 19–20    | 0.41              |
| ChatGPT, the bottom's gear and pill | 44        | 17–18    | 0.40              |

Both keep 16 from the screen's or the sheet's edge, and both draw a control
floating over content at 48 and one set in a surface at 44. That split is
seen, not written in the HIG, which states only the 44 target.

## What was tried

- A glyph at 0.4 of its container (19.2 in a 48, 17.6 in a 44): between the
  two references, but the strip's glyphs and the pill's beside them came out
  a few tenths apart, and the swatches lost their texture at 17.6.
- At 0.5: noticeably heavier than both references.
- A sheet's controls at 36, then 40 with a 44 target: 36 was under the
  target outright; 40 read well, and was dropped for 44 once every target
  was held to 44 and the sheet's controls matched Maps'.

## Where it stands

- `chrome.control` (48) is a map control's container (theme.ts `chrome`),
  `PANEL_CONTROL` (44, `chrome.target`) a sheet's (panel.tsx), and the strip's
  slots are 44 (dock.tsx `slot`), with what is lit in one a
  `chrome.selection` (36) capsule inside it, as a segmented control's
  selection is.
- `chrome.glyph` (20) is a control's glyph and the strip's tools'. The strip's
  swatches are 24 (dock.tsx `ITEM`), and its decor and marker glyphs 16 in a
  24 disc.
- CircularButton (circular-button.tsx) takes `size` and `place`, defaulting
  to a map control's, so a sheet's close is the same control at
  `PANEL_CONTROL`; its glyph is `chrome.glyph` whatever its size.
- Every gap between two controls is `chrome.controlGap` (8), and a sheet's
  controls stand 16 from its edges.
- A press wider than what one sees is a box laid out around it (a chip, the
  search field's clear, a toast; a pill's is pill.tsx `touchReach`), never a
  `hitSlop`: react-native-web honors `hitSlop` only on the old Touchable, so
  on the page a `Pressable`'s or a `View`'s landed on the shape alone, and a
  32 chip took 32.
