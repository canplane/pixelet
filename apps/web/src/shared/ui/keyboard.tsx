// @pxl/web/shared/ui/keyboard.tsx

import type { ReactNode } from "react";
import { Keyboard, StyleSheet, View, type ViewProps } from "react-native";

/** Whether the touch being dispatched began in a kept view. */
let kept = false;

/**
 * What to spread on a view a touch may start in and leave the keyboard up: a
 * field and what stands beside it to act on what is typed — a send, a clear.
 * A touch's start is offered to every view above the one touched, outermost
 * first, before any takes it; a kept view marks it as it passes and takes
 * nothing, so the view touched does what it does. Where a view lies is never
 * asked: a kept view may stand on the keyboard, lifted by a transform no
 * measure sees (KeyboardDock).
 */
export const KEEPS_KEYBOARD: ViewProps = { onStartShouldSetResponderCapture: () => keepKeyboard() };

/** Marks the touch being dispatched as one that leaves the keyboard up, and
 * takes nothing: for a view whose own capture handler is already another's
 * (a PanResponder's), to call from it. */
export function keepKeyboard(): false {
  kept = true;
  return false;
}

/**
 * The app's root for the keyboard: a touch that starts anywhere but in a kept
 * view is done typing, and puts the keyboard away — the map, a panel, a
 * button, all alike. It is offered the touch first, so it decides once the
 * offer has passed every view the touch began in.
 */
export function KeyboardAway({ children }: { children: ReactNode }) {
  const onTouch = () => {
    kept = false;
    queueMicrotask(() => {
      if (!kept) Keyboard.dismiss();
    });
    return false;
  };
  return <View onStartShouldSetResponderCapture={onTouch} style={styles.fill}>{children}</View>;
}

const styles = StyleSheet.create({
  fill: { flex: 1 },
});
