// @pxl/client/shell/progress-bar.tsx

import { StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";

import { Motion } from "@/shell/ui/motion/motion.tsx";
import { SETTLE } from "@/shell/ui/motion/transition.ts";
import { LABEL } from "@/shell/ui/palette.ts";

const WIDTH_PX = 160;
const HEIGHT_PX = 4;

/** `made`, 0 to 1: a bar filled that share. The fill slides in from the left
 * inside a track that clips it, so it eases between shares on the compositor. */
export function ProgressBar({ made }: { made: f64 }) {
  return (
    <View
      accessibilityRole="progressbar"
      accessibilityValue={{ min: 0, max: 100, now: Math.floor(made * 100) }}
      style={styles.track}
    >
      <Motion
        from={{ translateX: -WIDTH_PX }}
        to={{ translateX: (made - 1) * WIDTH_PX }}
        transition={SETTLE}
        style={styles.fill}
      />
    </View>
  );
}

const styles = StyleSheet.create({
  track: {
    width: WIDTH_PX,
    height: HEIGHT_PX,
    overflow: "hidden",
    backgroundColor: LABEL.tertiary,
  },
  fill: {
    width: WIDTH_PX,
    height: HEIGHT_PX,
    backgroundColor: LABEL.primary,
  },
});
