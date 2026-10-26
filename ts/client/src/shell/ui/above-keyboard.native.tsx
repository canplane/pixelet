// @pxl/client/shell/ui/above-keyboard.native.tsx

import { requireNativeView } from "expo";
import { type ComponentType, useState } from "react";
import type { NativeSyntheticEvent, ViewProps } from "react-native";

import { type AboveKeyboardProps, bottomRest } from "@/shell/ui/bottom-rest.ts";
import { layer } from "@/shell/ui/theme.ts";
import { bracket } from "@/bracket.tsx";

interface KeyboardDockProps extends ViewProps {
  clearance: number;
  rest: number;
  applied: number;
  onLift: (event: NativeSyntheticEvent<{ lift: number }>) => void;
}

const KeyboardDock: ComponentType<KeyboardDockProps> = requireNativeView(
  "PxlPlatform",
  "KeyboardDock",
);

/**
 * A layer standing `inset` above an open keyboard. The native view lifts it
 * in the keyboard's own animation, which a layout from JS a thread hop later
 * did not match; once still, the lift is laid out here so presses hit-test
 * where the layer is.
 */
export function AboveKeyboard(
  { inset, lift = 0, children }: AboveKeyboardProps,
) {
  const rest = bottomRest(bracket.view.useSafeArea(), inset);
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
        zIndex: layer.bottom,
        pointerEvents: "box-none",
      }}
    >
      {children}
    </KeyboardDock>
  );
}
