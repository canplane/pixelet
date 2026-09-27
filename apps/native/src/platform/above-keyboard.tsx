// @pxl/native/platform/above-keyboard.tsx

import { requireNativeView } from "expo";
import type { ComponentType, ReactNode } from "react";
import type { ViewProps } from "react-native";

import { bottomRest } from "@/shared/ui/bottom-rest.ts";

import { useSafeArea } from "./safe-area.ts";

interface AboveKeyboardProps {
  /** From the screen's side and bottom edges, and from the keyboard's top. */
  inset: number;
  /** Further up than `inset`, over what stands below it. */
  lift?: number;
  children: ReactNode;
}

/** The view that stands on the keyboard (modules/platform/ios/KeyboardDock.swift). */
const KeyboardDock: ComponentType<ViewProps & { clearance: number; rest: number }> = requireNativeView("PxlPlatform", "KeyboardDock");

/**
 * A layer across the bottom of the screen that stands `inset` over the
 * keyboard while one is open, and rests where the bottom chrome does while
 * none is (shared/ui/bottom-rest.ts). Laid out here where it rests, which
 * the native view is told (`rest`) so that it lifts from there. The lift is
 * the native view's own, moved in the keyboard's animation as the keyboard
 * says it is going, so the layer and the keys move as one — which a layout
 * set from here, a thread hop after the keys had left and on a curve of
 * React Native's, did not.
 */
export function AboveKeyboard({ inset, lift = 0, children }: AboveKeyboardProps) {
  const rest = bottomRest(useSafeArea(), inset);
  return (
    <KeyboardDock
      clearance={inset}
      rest={rest}
      style={{
        position: "absolute",
        left: inset,
        right: inset,
        bottom: rest + lift,
        zIndex: 40,
        pointerEvents: "box-none",
      }}
    >
      {children}
    </KeyboardDock>
  );
}
