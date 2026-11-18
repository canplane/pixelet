// @pxl/client/ui/feature/world/attribution.tsx

import { StyleSheet, Text, View } from "react-native";

import {
  alpha,
  COLOR,
  TEXT_SCALE_CAP,
  TEXT_STYLE,
  WEIGHT,
} from "@/ui/shared/component/theme.ts";
import { textHalo } from "@/ui/shared/component/typeface.ts";

const LINE_HEIGHT_PX = TEXT_STYLE.caption2.lineHeight;

/** The gap between the line and the dock under it. */
const GAP_PX = 8;

/** The most the line takes over the dock, its gap with it, at the largest
 * text the chrome takes (`TEXT_SCALE_CAP`): what the panels between the
 * chrome keep clear (world/view.tsx). */
export const ATTRIBUTION_HEIGHT_PX =
  GAP_PX + LINE_HEIGHT_PX * TEXT_SCALE_CAP.chrome;

/** Whose data the ground is, over the world wherever it is shown. The words
 * are the short form the OpenStreetMap Foundation's attribution guidelines
 * take over a map, `© OpenStreetMap`: the ODbL asks that whoever sees the
 * ground can tell where it came from, and the guidelines ask for that on the
 * map, seen without a touch. Overture's line and every other provider's whole
 * notice are under Settings, Attribution, where the license is linked.
 *
 * One line in the middle, `GAP_PX` over the dock, inside the dock and taking
 * no room in it: it rises over an open keyboard, dims and stands with the
 * dock, over the safe area and so never over the home indicator. The cards
 * and the size popover lie over it. Bare as the other words over the world
 * are, and taking no touch. */
export function Attribution() {
  return (
    <View style={STYLE.attribution}>
      <Text
        numberOfLines={1}
        maxFontSizeMultiplier={TEXT_SCALE_CAP.chrome}
        style={STYLE.line}
      >
        © OpenStreetMap
      </Text>
    </View>
  );
}

const STYLE = StyleSheet.create({
  attribution: {
    position: "absolute",
    left: 0,
    right: 0,
    bottom: "100%",
    marginBottom: GAP_PX,
    alignItems: "center",
    pointerEvents: "none",
  },
  line: {
    ...TEXT_STYLE.caption2,
    fontWeight: WEIGHT.regular,
    color: alpha(COLOR.white, 0.8),
    ...textHalo(alpha(COLOR.black, 0.9)),
  },
});
