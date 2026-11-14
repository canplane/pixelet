// @pxl/client/ui/feature/world/top/top.tsx

import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, View } from "react-native";
import { Defs, LinearGradient, Mask, Rect, Stop, Svg } from "react-native-svg";

import { open_modal } from "@/ui/shared/modal.ts";
import { Motion } from "@/ui/shared/component/motion/motion.tsx";
import { Presence } from "@/ui/shared/component/motion/presence.tsx";
import {
  LEAVE,
  SETTLE,
  SNAP,
} from "@/ui/shared/component/motion/transition.ts";
import { useDim } from "@/ui/shared/component/motion/dim.ts";
import { halo_gap, HaloIcon } from "./halo.tsx";
import { CHROME, COLOR, LAYER } from "@/ui/shared/component/theme.ts";
import { CurrentAddress } from "./current-address.tsx";
import { SensorIcons } from "./sensor-icon.tsx";
import { MapScaleBar } from "./scale-bar.tsx";
import { LocationStack } from "./location-stack.tsx";
import { ViewButtons } from "./view-button.tsx";
import { BusyDim } from "@/ui/feature/world/busy-dim.tsx";
import { useChromeTop } from "@/ui/feature/world/chrome.ts";
import { CircularButton } from "@/ui/shared/component/circular-button.tsx";
import { useWindowSize } from "@/service/platform/viewport.ts";
import { useCore } from "@/ui/shared/watch.ts";

export function TopOverlays() {
  const { t } = useTranslation();
  const top = useChromeTop();

  const { width } = useWindowSize();
  const scrim_depth = top + CHROME.control + SCRIM_TAIL;
  const from = (width - Math.min(width, SCRIM_WIDTH)) / 2 - SCRIM_FADE;
  const ramp = SCRIM_FADE / (width - 2 * from);

  return (
    <>
      <View style={[styles.scrim, { height: scrim_depth }]}>
        <Svg width="100%" height="100%">
          <Defs>
            <LinearGradient id="scrim-down" x1="0" y1="0" x2="0" y2="1">
              {SCRIM_EASE.map(([at, k]) => (
                <Stop
                  key={at}
                  offset={at}
                  stopColor={COLOR.black}
                  stopOpacity={SCRIM_OPACITY * (1 - k)}
                />
              ))}
            </LinearGradient>
            <LinearGradient
              id="scrim-across"
              gradientUnits="userSpaceOnUse"
              x1={from}
              y1={0}
              x2={width - from}
              y2={0}
            >
              {[
                ...SCRIM_EASE.map(([at, k]) => [ramp * at, k] as const),
                ...[...SCRIM_EASE]
                  .reverse()
                  .map(([at, k]) => [1 - ramp * at, k] as const),
              ].map(([at, k], i) => (
                <Stop
                  key={i}
                  offset={at}
                  stopColor={COLOR.white}
                  stopOpacity={k}
                />
              ))}
            </LinearGradient>
            <Mask id="scrim-mask">
              <Rect width="100%" height="100%" fill="url(#scrim-across)" />
            </Mask>
          </Defs>
          <Rect
            width="100%"
            height="100%"
            fill="url(#scrim-down)"
            mask="url(#scrim-mask)"
          />
        </Svg>
      </View>

      <BusyDim style={[styles.left, { top }]}>
        <CircularButton
          glyph="line.3.horizontal"
          onPress={() => open_modal("settings")}
          label={t("settings.trigger_aria_label")}
        />
      </BusyDim>

      <BusyDim style={[styles.center, { top }]}>
        <SearchStrip />
      </BusyDim>

      <BusyDim style={[styles.right, { top }]}>
        <LocationStack />
        <ViewButtons />
      </BusyDim>
    </>
  );
}

function SearchStrip() {
  const { t } = useTranslation();
  const zooming = useCore((s) => s.zooming);
  const dim = useDim(1, SNAP);
  return (
    <Motion {...dim.motion} style={styles.fill}>
      <Pressable
        onPress={() => open_modal("search")}
        accessibilityRole="button"
        accessibilityLabel={t("world.search.trigger_aria_label")}
        style={styles.strip}
      >
        <Presence
          show={!zooming}
          from={{ opacity: 0 }}
          exit={{ opacity: 0 }}
          transition={SETTLE}
          exitTransition={LEAVE}
          style={styles.place}
        >
          <View style={styles.title}>
            <View style={styles.search}>
              <HaloIcon
                glyph="magnifyingglass"
                size={12}
                weight="semibold"
                color={COLOR.white}
              />
              <CurrentAddress />
            </View>
            <SensorIcons />
          </View>
        </Presence>
        <View style={styles.scale}>
          <MapScaleBar show={zooming} />
        </View>
      </Pressable>
    </Motion>
  );
}

/** A dark scrim along the top, so that white chrome reads over a bright world
 * without a blur. It is flat across `SCRIM_WIDTH` and then fades out along a
 * smoothstep, because the end of a straight fade reads as an edge. */
const SCRIM_OPACITY = 0.45;
const SCRIM_WIDTH = 560;
const SCRIM_FADE = 240;
const SCRIM_TAIL = 32;
const SCRIM_EASE = Array.from({ length: 9 }, (_, i) => {
  const t = i / 8;
  return [t, t * t * (3 - 2 * t)] as const;
});

const styles = StyleSheet.create({
  scrim: {
    position: "absolute",
    top: 0,
    left: 0,
    right: 0,
    zIndex: LAYER.scrim,
    pointerEvents: "none",
  },
  left: {
    position: "absolute",
    left: CHROME.inset,
    zIndex: LAYER.chrome,
    alignItems: "flex-start",
    gap: 12,
  },
  center: {
    position: "absolute",
    left: 0,
    right: 0,
    paddingHorizontal: CHROME.inset + CHROME.control + CHROME.control_gap,
    minHeight: CHROME.control,
    zIndex: LAYER.chrome,
    alignItems: "center",
    justifyContent: "center",
  },
  right: {
    position: "absolute",
    right: CHROME.inset,
    zIndex: LAYER.chrome,
    alignItems: "center",
    gap: CHROME.control_gap,
  },
  fill: { alignSelf: "stretch", flex: 1 },
  // The whole strip takes the press, whether the scale or the address shows.
  strip: { flex: 1, alignItems: "center", justifyContent: "center" },
  place: { maxWidth: "100%" },
  title: { alignItems: "center", gap: halo_gap(8) },
  // The row is bounded, because a page lays out a row that does not wrap at
  // its content's width, and a long address would run over the buttons.
  search: {
    flexDirection: "row",
    alignItems: "center",
    gap: halo_gap(8),
    minWidth: 0,
    maxWidth: "100%",
    flexShrink: 1,
  },
  scale: {
    ...StyleSheet.absoluteFill,
    alignItems: "center",
    justifyContent: "center",
    pointerEvents: "none",
  },
});
