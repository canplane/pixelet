// @pxl/client/shell/progress-bar.tsx

import { StyleSheet, View } from "react-native";

import type { f64 } from "@xpute/kit/abi/word.ts";

import { Motion } from "@/shell/ui/motion/motion.tsx";
import { SETTLE } from "@/shell/ui/motion/transition.ts";
import { color } from "@/shell/ui/theme.ts";

const WIDTH_PX = 160;
const HEIGHT_PX = 4;

/** `share`, 0 to 1: a bar filled that much. The fill slides in from the left
 * inside a track that clips it, so it eases between shares on the compositor. */
export function ProgressBar({ share }: { share: f64 }) {
  return (
    <View
      accessibilityRole="progressbar"
      accessibilityValue={{ min: 0, max: 100, now: Math.floor(share * 100) }}
      style={styles.track}
    >
      <Motion
        from={{ translateX: -WIDTH_PX }}
        to={{ translateX: (share - 1) * WIDTH_PX }}
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
    backgroundColor: color.track,
  },
  fill: {
    width: WIDTH_PX,
    height: HEIGHT_PX,
    backgroundColor: color.chosen,
  },
});
