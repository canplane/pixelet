// @pxl/native/platform/above-keyboard.tsx

import { requireNativeView } from "expo";
import type { ComponentType, ReactNode } from "react";
import type { ViewProps } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

interface AboveKeyboardProps {
  /** From the screen's side and bottom edges, and from the keyboard's top. */
  inset: number;
  /** Further up than `inset`, over what stands below it. */
  lift?: number;
  children: ReactNode;
}

/** The view that stands on the keyboard (modules/platform/ios/KeyboardDock.swift). */
const KeyboardDock: ComponentType<ViewProps> = requireNativeView("PxlPlatform", "KeyboardDock");

/**
 * A layer across the bottom of the screen that stands on the keyboard while
 * one is open, and over the home indicator while none is. Laid out here where
 * it stands with no keyboard; the keyboard's lift is the native view's own,
 * moved in the keyboard's animation as the keyboard says it is going, so the
 * layer and the keys move as one — which a layout set from here, a thread hop
 * after the keys had left and on a curve of React Native's, did not.
 */
export function AboveKeyboard({ inset, lift = 0, children }: AboveKeyboardProps) {
  const { bottom } = useSafeAreaInsets();
  return (
    <KeyboardDock
      style={{
        position: "absolute",
        left: inset,
        right: inset,
        bottom: inset + lift + bottom,
        zIndex: 40,
        pointerEvents: "box-none",
      }}
    >
      {children}
    </KeyboardDock>
  );
}
