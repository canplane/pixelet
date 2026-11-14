// @pxl/client/ui/feature/cover/progress-bar.tsx

import { StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";

import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import { SETTLE } from "@/ui/shared/component/motion/transition.ts";
import { COLOR } from "@/ui/shared/component/theme.ts";

const WIDTH_PX = 160;
const HEIGHT_PX = 4;

/** A bar filled to `share`, from 0 to 1. The fill is as wide as the track and
 * slides in from the left inside it, clipped by it, so that the fill eases
 * between shares by a transform on the compositor. */
export function ProgressBar({ share }: { share: f64 }) {
  return (
    <View
      accessibilityRole="progressbar"
      accessibilityValue={{ min: 0, max: 100, now: Math.floor(share * 100) }}
      style={STYLE.track}
    >
      <Motion
        from={{ translateX: -WIDTH_PX }}
        to={{ translateX: (share - 1) * WIDTH_PX }}
        transition={SETTLE}
        style={STYLE.fill}
      />
    </View>
  );
}

const STYLE = StyleSheet.create({
  track: {
    width: WIDTH_PX,
    height: HEIGHT_PX,
    overflow: "hidden",
    backgroundColor: COLOR.track,
  },
  fill: {
    width: WIDTH_PX,
    height: HEIGHT_PX,
    backgroundColor: COLOR.chosen,
  },
});
