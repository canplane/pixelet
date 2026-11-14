// @pxl/client/ui/shared/component/above-keyboard.native.tsx

import { requireNativeView } from "expo";
import { type ComponentType, useState } from "react";
import type { NativeSyntheticEvent, ViewProps } from "react-native";

import {
  type AboveKeyboardProps,
  bottom_rest,
} from "@/ui/shared/component/bottom-rest.ts";
import { LAYER } from "@/ui/shared/component/theme.ts";
import { useSafeArea } from "@/service/platform/safe-area.ts";

interface KeyboardDockProps extends ViewProps {
  clearance: number;
  rest: number;
  layoutLiftPx: number;
  onLift: (event: NativeSyntheticEvent<{ lift: number }>) => void;
}

const KeyboardDock: ComponentType<KeyboardDockProps> = requireNativeView(
  "PxlPlatform",
  "KeyboardDock",
);

/**
 * The layer stands `inset` above an open keyboard. The native view lifts it
 * within the keyboard's own animation. Once the keyboard is still, the native
 * view reports the lift and the layout here takes it on, so that presses
 * hit-test where the layer is drawn.
 */
export function AboveKeyboard({
  inset,
  lift = 0,
  children,
}: AboveKeyboardProps) {
  const rest = bottom_rest(useSafeArea(), inset);
  const [layout_lift, set_layout_lift] = useState(0);
  return (
    <KeyboardDock
      clearance={inset}
      rest={rest}
      layoutLiftPx={layout_lift}
      onLift={(event) => set_layout_lift(event.nativeEvent.lift)}
      style={{
        position: "absolute",
        left: inset,
        right: inset,
        bottom: rest + lift + layout_lift,
        zIndex: LAYER.bottom,
        pointerEvents: "box-none",
      }}
    >
      {children}
    </KeyboardDock>
  );
}
