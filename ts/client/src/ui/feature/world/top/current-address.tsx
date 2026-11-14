// @pxl/client/ui/feature/world/top/current-address.tsx

import { StyleSheet } from "react-native";
import { useTranslation } from "react-i18next";

import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import {
  useDivisionLabel,
  usePivotCoordinate,
} from "@/ui/feature/division/pivot.ts";
import { FADE } from "@/ui/shared/component/motion/transition.ts";
import { HaloText } from "./halo.tsx";
import {
  alpha,
  COLOR,
  SECONDARY,
  TEXT_STYLE,
  WEIGHT,
} from "@/ui/shared/component/theme.ts";

export function CurrentAddress() {
  const { t } = useTranslation();
  const label = useDivisionLabel();
  const position = usePivotCoordinate();

  const text = label ?? position;

  return (
    <Motion
      key={label ?? (position === null ? "locating" : "")}
      from={{ opacity: 0 }}
      to={{}}
      transition={FADE}
      style={STYLE.shrink}
    >
      <HaloText
        style={[STYLE.label, !label && STYLE.numbers, !text && STYLE.locating]}
      >
        {text ?? t("world.search.locating")}
      </HaloText>
    </Motion>
  );
}

const STYLE = StyleSheet.create({
  shrink: { flexShrink: 1, minWidth: 0 },
  label: {
    ...TEXT_STYLE.body,
    fontWeight: WEIGHT.semibold,
    color: COLOR.white,
  },
  numbers: { fontVariant: ["tabular-nums"] },
  locating: { color: alpha(COLOR.white, SECONDARY) },
});
