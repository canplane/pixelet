// @pxl/native/platform/above-keyboard.tsx

import { type ReactNode, useEffect, useState } from "react";
import { Keyboard, Platform, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

interface AboveKeyboardProps {
  /** From the screen's side and bottom edges, and from the keyboard's top. */
  inset: number;
  /** Further up than `inset`, over what stands below it. */
  lift?: number;
  children: ReactNode;
}

/**
 * A layer across the bottom of the screen that stands on the keyboard while
 * one is open, and over the home indicator while none is. iOS says where the
 * keyboard is going before it moves, Android once it has.
 */
export function AboveKeyboard({ inset, lift = 0, children }: AboveKeyboardProps) {
  const { bottom } = useSafeAreaInsets();
  const [keyboard, setKeyboard] = useState(0);

  useEffect(() => {
    const show = Platform.OS === "ios" ? "keyboardWillShow" : "keyboardDidShow";
    const hide = Platform.OS === "ios" ? "keyboardWillHide" : "keyboardDidHide";
    const shown = Keyboard.addListener(show, (e) => setKeyboard(e.endCoordinates.height));
    const hidden = Keyboard.addListener(hide, () => setKeyboard(0));
    return () => {
      shown.remove();
      hidden.remove();
    };
  }, []);

  return (
    <View
      style={{
        position: "absolute",
        left: inset,
        right: inset,
        bottom: inset + lift + Math.max(keyboard, bottom),
        zIndex: 40,
        pointerEvents: "box-none",
      }}
    >
      {children}
    </View>
  );
}
