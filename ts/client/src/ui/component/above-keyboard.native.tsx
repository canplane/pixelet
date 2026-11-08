// @pxl/client/ui/component/above-keyboard.native.tsx

import { requireNativeView } from "expo";
import { type ComponentType, useState } from "react";
import type { NativeSyntheticEvent, ViewProps } from "react-native";

import {
  type AboveKeyboardProps,
  bottomRest,
} from "@/ui/component/bottom-rest.ts";
import { layer } from "@/ui/component/theme.ts";
import { useSafeArea } from "@/service/platform/safe-area.ts";

interface KeyboardDockProps extends ViewProps {
  clearance: number;
  rest: number;
  layoutLift: number;
  onLift: (event: NativeSyntheticEvent<{ lift: number }>) => void;
}

const KeyboardDock: ComponentType<KeyboardDockProps> = requireNativeView(
  "PxlPlatform",
  "KeyboardDock",
);

/**
 * A layer on a device that stands `inset` above an open keyboard. The native
 * view lifts the layer within the keyboard's own animation. Once the keyboard
 * is still, the native view reports the lift and the layout here takes it on,
 * so that presses hit-test where the layer is drawn.
 */
export function AboveKeyboard({
  inset,
  lift = 0,
  children,
}: AboveKeyboardProps) {
  const rest = bottomRest(useSafeArea(), inset);
  const [layoutLift, setLayoutLift] = useState(0);
  return (
    <KeyboardDock
      clearance={inset}
      rest={rest}
      layoutLift={layoutLift}
      onLift={(event) => setLayoutLift(event.nativeEvent.lift)}
      style={{
        position: "absolute",
        left: inset,
        right: inset,
        bottom: rest + lift + layoutLift,
        zIndex: layer.bottom,
        pointerEvents: "box-none",
      }}
    >
      {children}
    </KeyboardDock>
  );
}
