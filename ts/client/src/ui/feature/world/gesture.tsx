// @pxl/client/ui/feature/world/gesture.tsx

/**
 * The hand's gestures on the world, recognized alike on the web and natively
 * by react-native-gesture-handler, and what each asks of the camera. A finger
 * or a mouse's primary button pans, and taps where it lets go without having
 * moved. A mouse's secondary button, or the primary with Shift, turns the
 * view about the screen's center as an arcball does, or tilts it, by
 * whichever axis its drag takes first. Two fingers pinch to zoom, twist to
 * turn and move up or down together to tilt, and do not pan.
 *
 * A press that starts on the world puts the keyboard away. While the dock is
 * in message mode, that press ends the typing instead of tapping, and its
 * drag still moves the world. In message mode the pointer counts as off the
 * world, so that nothing hovers there and no tool previews what a tap would
 * do.
 *
 * Every gesture runs on the JS thread (`runOnJS`), where the core is sent
 * to; react-native-reanimated would else run its callbacks on the UI's. Its
 * Babel plugin still hands each callback the values around it as they stood
 * when the callback was made, so a callback only passes its event to a
 * function here, and the functions alone read and write what this file
 * holds.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { wrap } from "@xpute/kit/math/scalar.ts";
import type { ReactNode } from "react";
import {
  type LayoutChangeEvent,
  type ViewStyle,
  type StyleProp,
  View,
} from "react-native";
import {
  GestureDetector,
  MouseButton,
  PointerType,
  useHoverGesture,
  usePanGesture,
  usePinchGesture,
  useRotationGesture,
  useSimultaneousGestures,
  useTapGesture,
} from "react-native-gesture-handler";

import { putKeyboardAway } from "@/ui/shared/component/keyboard.tsx";
import {
  DockMode,
  dockMode,
  setDockMode,
  watchDockMode,
} from "@/ui/feature/world/dock/store.ts";
import { pressShifted } from "@/service/platform/surface.tsx";
import * as input from "@/service/core/input.ts";

/** How far a press may move and still tap, as real pointers jitter a pixel or
 * two on a still click; a pan starts past it. */
const TAP_SLOP_PX = 4;
/** The share of the view's height a drag crosses to tilt the view by π. */
const TILT_SWEEP_FRACTION = 0.5;
/** Once a turning drag moves this far, its dominant axis holds for the rest
 * of the drag, so that an off-center vertical drag does not start to turn
 * the view once the tilt reaches its limit. */
const AXIS_LOCK_PX = 12;
/** Within this radius of the screen's center, a turning drag does not turn
 * the view, because the hand's angle about the center swings wildly there. */
const MIN_TURN_RADIUS_PX = 24;

/** The view's size, in its logical pixels. */
let size: [f64, f64] = [0, 0];

/** Whether a finger or the primary button is pressed on the world, and how
 * many fingers are, as the primary drag's events last said. */
let pressed = false;
let fingerCount = 0;

/** Whether the press under way taps when it lets go: not one that ended the
 * typing, and not one a second finger joined. */
let taps = false;

/** The drag under way turns rather than pans, with the axis it took, the
 * hand's angle about the center (null near the center) and its height. */
let turning = false;
let axis: "none" | "tilt" | "turn" = "none";
let from: [f64, f64] = [0, 0];
let angle: f64 | null = null;
let lastY = 0;

watchDockMode((mode) => {
  if (mode === DockMode.MESSAGE) input.out();
});

function sendDragMode(): void {
  input.setDragMode(
    fingerCount > 1 ? "pinch" : turning ? "turn" : pressed ? "pan" : "none",
  );
}

/** The tilt a hand's travel of `dy` pixels asks for, in radians: negative,
 * because a hand moving up tilts toward the horizon. */
function tiltOf(dy: f64): f64 {
  return (-Math.PI * dy) / (TILT_SWEEP_FRACTION * size[1]);
}

function beginTurn(x: f64, y: f64): void {
  turning = true;
  axis = "none";
  from = [x, y];
  angle = null;
  lastY = y;
}

function turnTo(x: f64, y: f64): void {
  if (axis === "none") {
    const [dx, dy] = [x - from[0], y - from[1]];
    if (Math.hypot(dx, dy) < AXIS_LOCK_PX) return;
    axis = Math.abs(dy) > Math.abs(dx) ? "tilt" : "turn";
  }
  if (axis === "tilt") {
    input.tilt(tiltOf(y - lastY));
    lastY = y;
    return;
  }
  const [cx, cy] = [x - size[0] / 2, y - size[1] / 2];
  if (Math.hypot(cx, cy) < MIN_TURN_RADIUS_PX) {
    angle = null;
    return;
  }
  const now = Math.atan2(cy, cx);
  if (angle !== null) input.turn(-wrap(now - angle, -Math.PI, Math.PI));
  angle = now;
}

function beginPress(x: f64, y: f64): void {
  pressed = true;
  putKeyboardAway();
  taps = dockMode() !== DockMode.MESSAGE;
  if (!taps) setDockMode(DockMode.PALETTE);
  if (pressShifted()) beginTurn(x, y);
  sendDragMode();
}

function setFingerCount(count: f64): void {
  fingerCount = count;
  if (fingerCount > 1) taps = false;
  sendDragMode();
}

/** The drag of every finger down, at their middle: one finger pans, and two
 * moving up or down together tilt. */
function dragTo(x: f64, y: f64, dx: f64, dy: f64, count: f64): void {
  setFingerCount(count);
  if (fingerCount > 1) input.tilt(tiltOf(dy));
  else if (turning) turnTo(x, y);
  else input.pan(dx, dy);
}

function endPress(): void {
  pressed = false;
  fingerCount = 0;
  turning = false;
  sendDragMode();
}

function beginSecondaryPress(x: f64, y: f64): void {
  beginTurn(x, y);
  sendDragMode();
}

function endSecondaryPress(): void {
  turning = false;
  sendDragMode();
}

function tapAt(x: f64, y: f64): void {
  if (taps && !pressShifted()) input.tap(x, y);
}

function hoverAt(x: f64, y: f64): void {
  if (dockMode() !== DockMode.MESSAGE) input.hover(x, y);
}

export function WorldGestures({
  style,
  children,
}: {
  style: StyleProp<ViewStyle>;
  children: ReactNode;
}) {
  // A pinch and a twist are told apart from the drag only once they pass
  // their own thresholds, so the drag counts the fingers.
  const primary = usePanGesture({
    runOnJS: true,
    minDistance: TAP_SLOP_PX,
    averageTouches: true,
    mouseButton: MouseButton.LEFT,
    onBegin: (e) => beginPress(e.x, e.y),
    onTouchesDown: (e) => setFingerCount(e.numberOfTouches),
    onUpdate: (e) => dragTo(e.x, e.y, e.changeX, e.changeY, e.numberOfPointers),
    onFinalize: () => endPress(),
  });
  // A button picks among a mouse's presses alone, so a finger is left to
  // the primary.
  const secondary = usePanGesture({
    runOnJS: true,
    minDistance: TAP_SLOP_PX,
    mouseButton: MouseButton.RIGHT,
    onBegin: (e) => {
      if (e.pointerType === PointerType.MOUSE) beginSecondaryPress(e.x, e.y);
    },
    onUpdate: (e) => {
      if (e.pointerType === PointerType.MOUSE) turnTo(e.x, e.y);
    },
    onFinalize: (e) => {
      if (e.pointerType === PointerType.MOUSE) endSecondaryPress();
    },
  });
  const pinch = usePinchGesture({
    runOnJS: true,
    onUpdate: (e) => input.zoom(e.scaleChange),
  });
  const twist = useRotationGesture({
    runOnJS: true,
    onUpdate: (e) => input.turn(-e.rotationChange),
  });
  const tap = useTapGesture({
    runOnJS: true,
    maxDistance: TAP_SLOP_PX,
    onActivate: (e) => tapAt(e.x, e.y),
  });
  const hover = useHoverGesture({
    runOnJS: true,
    onUpdate: (e) => hoverAt(e.x, e.y),
    onFinalize: () => input.out(),
  });
  const gestures = useSimultaneousGestures(
    primary,
    secondary,
    pinch,
    twist,
    tap,
    hover,
  );

  return (
    <GestureDetector gesture={gestures}>
      <View
        style={style}
        onLayout={(e: LayoutChangeEvent) => {
          const { width, height } = e.nativeEvent.layout;
          size = [width, height];
        }}
      >
        {children}
      </View>
    </GestureDetector>
  );
}
