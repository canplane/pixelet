// @pxl/client/ui/component/keyboard.tsx

import type { ReactNode } from "react";
import { Keyboard, StyleSheet, View, type ViewProps } from "react-native";

let keep = false;

/**
 * Spread these props on a view where a touch should leave the keyboard as it
 * is: still up, or put away by the view itself. The view marks the touch
 * during the capture phase and claims nothing. A touch is never judged by its
 * position, because a transform can lift a view where no measurement sees it.
 */
export const KEEPS_KEYBOARD: ViewProps = {
  onStartShouldSetResponderCapture: () => keepKeyboard(),
};

export function putKeyboardAway(): void {
  Keyboard.dismiss();
}

/** Marks the current touch as one that keeps the keyboard. A view whose
 * capture handler is already a PanResponder's calls this from that handler
 * instead of spreading KEEPS_KEYBOARD. */
export function keepKeyboard(): false {
  keep = true;
  return false;
}

/** A touch that starts anywhere but in a view that keeps the keyboard puts
 * the keyboard away and calls `onAway`. */
export function KeyboardAway({
  children,
  onAway,
}: {
  children: ReactNode;
  onAway?: () => void;
}) {
  const onTouch = () => {
    // Capture runs from the outside in, so this handler runs before any inner
    // view can mark the touch. The decision waits for a microtask, by which
    // time every inner view has had its turn.
    keep = false;
    queueMicrotask(() => {
      if (keep) return;
      putKeyboardAway();
      onAway?.();
    });
    return false;
  };
  return (
    <View onStartShouldSetResponderCapture={onTouch} style={styles.fill}>
      {children}
    </View>
  );
}

const styles = StyleSheet.create({
  fill: { flex: 1 },
});
