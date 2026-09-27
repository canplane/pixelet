// @pxl/native/platform/above-keyboard.tsx

import { requireNativeView } from "expo";
import { type ComponentType, type ReactNode, useState } from "react";
import type { NativeSyntheticEvent, ViewProps } from "react-native";

import { bottomRest } from "@/shared/ui/bottom-rest.ts";

import { useSafeArea } from "./safe-area.ts";

interface AboveKeyboardProps {
  /** From the screen's side and bottom edges, and from the keyboard's top. */
  inset: number;
  /** Further up than `inset`, over what stands below it. */
  lift?: number;
  children: ReactNode;
}

interface KeyboardDockProps extends ViewProps {
  clearance: number;
  rest: number;
  /** How much of the lift this layout carries. */
  applied: number;
  /** The lift, once the keyboard is still. */
  onLift: (event: NativeSyntheticEvent<{ lift: number }>) => void;
}

/** The view that stands on the keyboard (modules/platform/ios/KeyboardDock.swift). */
const KeyboardDock: ComponentType<KeyboardDockProps> = requireNativeView("PxlPlatform", "KeyboardDock");

/**
 * A layer across the bottom of the screen that stands `inset` over the
 * keyboard while one is open, and rests where the bottom chrome does while
 * none is (shared/ui/bottom-rest.ts). Laid out here where it rests, which
 * the native view is told (`rest`) so that it lifts from there. The lift is
 * the native view's own while the keyboard moves, in the keyboard's
 * animation as the keyboard says it is going, so the layer and the keys move
 * as one — which a layout set from here, a thread hop after the keys had
 * left and on a curve of React Native's, did not. Once the keys are still,
 * the lift is laid out here (`applied`) and the native view's own is none,
 * so what React Native measures a press against is where the layer is
 * (KeyboardDock).
 */
export function AboveKeyboard({ inset, lift = 0, children }: AboveKeyboardProps) {
  const rest = bottomRest(useSafeArea(), inset);
  const [applied, setApplied] = useState(0);
  return (
    <KeyboardDock
      clearance={inset}
      rest={rest}
      applied={applied}
      onLift={(event) => setApplied(event.nativeEvent.lift)}
      style={{
        position: "absolute",
        left: inset,
        right: inset,
        bottom: rest + lift + applied,
        zIndex: 40,
        pointerEvents: "box-none",
      }}
    >
      {children}
    </KeyboardDock>
  );
}
