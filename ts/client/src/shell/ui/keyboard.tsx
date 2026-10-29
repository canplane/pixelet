// @pxl/client/shell/ui/keyboard.tsx

import type { ReactNode } from "react";
import { Keyboard, StyleSheet, View, type ViewProps } from "react-native";

let kept = false;

/**
 * Spread on a view a touch may start in and leave the keyboard as it is,
 * up or put away by the view itself. A kept view marks the touch during
 * capture and takes nothing; position is never asked, since a view may be
 * lifted by a transform no measure sees.
 */
export const KEEPS_KEYBOARD: ViewProps = {
  onStartShouldSetResponderCapture: () => keepKeyboard(),
};

/** Puts the keyboard away, wherever it is up. */
export function putKeyboardAway(): void {
  Keyboard.dismiss();
}

/** For a view whose capture handler is already a PanResponder's. */
export function keepKeyboard(): false {
  kept = true;
  return false;
}

/** A touch starting anywhere but a kept view puts the keyboard away, and `onAway` is told. */
export function KeyboardAway({
  children,
  onAway,
}: {
  children: ReactNode;
  onAway?: () => void;
}) {
  const onTouch = () => {
    kept = false;
    queueMicrotask(() => {
      if (kept) return;
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
