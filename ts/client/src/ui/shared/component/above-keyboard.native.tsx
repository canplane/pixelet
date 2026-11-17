// @pxl/client/ui/shared/component/above-keyboard.native.tsx

import { StyleSheet } from "react-native";
import { useReanimatedKeyboardAnimation } from "react-native-keyboard-controller";
import Animated, { useAnimatedStyle } from "react-native-reanimated";

import {
  type AboveKeyboardProps,
  bottomRest,
} from "@/ui/shared/component/bottom-rest.ts";
import { LAYER } from "@/ui/shared/component/theme.ts";
import { useSafeArea } from "@/service/platform/safe-area.ts";

/**
 * The layer stands `inset` above an open keyboard, or where it rests with none
 * up. react-native-keyboard-controller gives the keyboard's height on the UI
 * thread as the keyboard moves, each frame on Android and its end inside the
 * keyboard's own animation on iOS, and the layer is laid out from it, so that
 * the layer and the keys move as one and presses hit-test where the layer is
 * drawn.
 */
export function AboveKeyboard({
  inset,
  lift = 0,
  children,
}: AboveKeyboardProps) {
  const rest = bottomRest(useSafeArea(), inset);
  // The height is zero with no keyboard up and the keyboard's own, negated,
  // with one up.
  const { height } = useReanimatedKeyboardAnimation();
  const bottom = useAnimatedStyle(() => ({
    bottom: Math.max(rest, inset - height.value) + lift,
  }));
  return (
    <Animated.View
      pointerEvents="box-none"
      style={[STYLE.layer, { left: inset, right: inset }, bottom]}
    >
      {children}
    </Animated.View>
  );
}

const STYLE = StyleSheet.create({
  layer: { position: "absolute", zIndex: LAYER.bottom },
});
