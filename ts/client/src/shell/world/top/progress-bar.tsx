// @pxl/client/shell/world/top/progress-bar.tsx

import { StyleSheet, View } from "react-native";
import { Svg } from "react-native-svg";

import type { f64 } from "@xpute/kit/abi/word.ts";

import { Motion } from "@/shell/ui/motion/motion.tsx";
import { Presence } from "@/shell/ui/motion/presence.tsx";
import { LEAVE, SETTLE } from "@/shell/ui/motion/transition.ts";
import { HaloPath, HaloText } from "./halo.tsx";
import { BAR_MAX_PX, LABEL_GAP_PX, scaleLabel } from "./scale-bar.tsx";
import { color, halo } from "@/shell/ui/theme.ts";
import { useKernel } from "@/shell/store/watch.ts";

const LINE_PX = 1;
const HEIGHT_PX = 7;
// Between the outline and the fill.
const GAP_PX = 1;
const PAD_PX = LINE_PX + halo.width + 1;
const WIDTH = BAR_MAX_PX + 2 * PAD_PX;
const SVG_HEIGHT = 2 * PAD_PX + HEIGHT_PX;
const INSET = LINE_PX + GAP_PX;
const FILL_WIDTH = BAR_MAX_PX - 2 * INSET;
const FILL_HEIGHT = HEIGHT_PX - 2 * INSET;

/** A capsule `w` by `h` at (`x`, `y`), its ends half circles. */
function capsule(x: f64, y: f64, w: f64, h: f64): string {
  const r = h / 2;
  return (
    `M${x + r} ${y}H${x + w - r}A${r} ${r} 0 0 1 ${x + w - r} ${y + h}` +
    `H${x + r}A${r} ${r} 0 0 1 ${x + r} ${y}Z`
  );
}

/** `renderer_progress` in the scale bar's place: a capsule BAR_MAX_PX wide,
 * its top where the scale's line is, filled that share, its percent above it
 * where the scale's label is. The fill slides in from the left inside a
 * capsule that clips it, so it eases between shares on the compositor. */
export function ProgressBar({ show }: { show: boolean }) {
  const made = useKernel((s) => s.renderer_progress);
  return (
    <Presence
      show={show}
      from={{ opacity: 0 }}
      exit={{ opacity: 0 }}
      transition={SETTLE}
      exitTransition={LEAVE}
      style={styles.root}
    >
      <View style={styles.above}>
        <HaloText style={scaleLabel}>{`${Math.floor(made * 100)}%`}</HaloText>
      </View>
      <View>
        <Svg
          width={WIDTH}
          height={SVG_HEIGHT}
          viewBox={`0 0 ${WIDTH} ${SVG_HEIGHT}`}
        >
          <HaloPath
            d={capsule(
              PAD_PX + 0.5,
              PAD_PX + 0.5,
              BAR_MAX_PX - LINE_PX,
              HEIGHT_PX - LINE_PX,
            )}
            width={LINE_PX}
          />
        </Svg>
        <View style={styles.clip}>
          <Motion
            from={{ translateX: -FILL_WIDTH }}
            to={{ translateX: (made - 1) * FILL_WIDTH }}
            transition={SETTLE}
            style={styles.fill}
          />
        </View>
      </View>
    </Presence>
  );
}

const styles = StyleSheet.create({
  clip: {
    position: "absolute",
    left: PAD_PX + INSET,
    top: PAD_PX + INSET,
    width: FILL_WIDTH,
    height: FILL_HEIGHT,
    borderRadius: FILL_HEIGHT / 2,
    overflow: "hidden",
  },
  fill: {
    width: FILL_WIDTH,
    height: FILL_HEIGHT,
    borderRadius: FILL_HEIGHT / 2,
    backgroundColor: color.white,
  },
  // The outline's top at the strip's middle, as the scale's line.
  root: { alignItems: "center", paddingTop: HEIGHT_PX },
  // The percent's glyphs the scale label's gap above the outline.
  above: {
    position: "absolute",
    left: 0,
    right: 0,
    bottom: HEIGHT_PX + PAD_PX + LABEL_GAP_PX - halo.width,
    alignItems: "center",
  },
});
