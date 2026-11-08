// @pxl/client/ui/world/top/top.tsx

import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, View } from "react-native";
import { Defs, LinearGradient, Mask, Rect, Stop, Svg } from "react-native-svg";

import { openModal } from "@/ui/modal.tsx";
import { Motion } from "@/ui/component/motion/motion.tsx";
import { Presence } from "@/ui/component/motion/presence.tsx";
import { LEAVE, SETTLE, SNAP } from "@/ui/component/motion/transition.ts";
import { useDim } from "@/ui/component/motion/dim.ts";
import { haloGap, HaloIcon } from "./halo.tsx";
import { chrome, color, layer } from "@/ui/component/theme.ts";
import { CurrentAddress } from "./current-address.tsx";
import { SensorIcons } from "./sensor-icons.tsx";
import { MapScaleBar } from "./scale-bar.tsx";
import { LocationStack } from "./location-stack.tsx";
import { ViewButtons } from "./view-buttons.tsx";
import { BusyDim } from "@/ui/world/busy-dim.tsx";
import { useChromeTop } from "@/ui/world/chrome.ts";
import { CircularButton } from "@/ui/component/circular-button.tsx";
import { useWindowSize } from "@/service/platform/viewport.ts";
import { useCore } from "@/ui/watch.ts";

export function TopOverlays() {
  const { t } = useTranslation();
  const top = useChromeTop();

  const { width } = useWindowSize();
  const scrimDepth = top + chrome.control + SCRIM_TAIL;
  const from = (width - Math.min(width, SCRIM_WIDTH)) / 2 - SCRIM_FADE;
  const ramp = SCRIM_FADE / (width - 2 * from);

  return (
    <>
      <View style={[styles.scrim, { height: scrimDepth }]}>
        <Svg width="100%" height="100%">
          <Defs>
            <LinearGradient id="scrim-down" x1="0" y1="0" x2="0" y2="1">
              {SCRIM_EASE.map(([at, k]) => (
                <Stop
                  key={at}
                  offset={at}
                  stopColor={color.black}
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
                  stopColor={color.white}
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
          onPress={() => openModal("settings")}
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

/** The strip at the top center. It shows the map's scale while zooming and
 * the address otherwise, and it dims with the buttons beside it. */
function SearchStrip() {
  const { t } = useTranslation();
  const zooming = useCore((s) => s.zooming);
  const dim = useDim(1, SNAP);
  return (
    <Motion {...dim.motion} style={styles.fill}>
      <Pressable
        onPress={() => openModal("search")}
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
                color={color.white}
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
    zIndex: layer.scrim,
    pointerEvents: "none",
  },
  left: {
    position: "absolute",
    left: chrome.inset,
    zIndex: layer.chrome,
    alignItems: "flex-start",
    gap: 12,
  },
  center: {
    position: "absolute",
    left: 0,
    right: 0,
    paddingHorizontal: chrome.inset + chrome.control + chrome.controlGap,
    minHeight: chrome.control,
    zIndex: layer.chrome,
    alignItems: "center",
    justifyContent: "center",
  },
  right: {
    position: "absolute",
    right: chrome.inset,
    zIndex: layer.chrome,
    alignItems: "center",
    gap: chrome.controlGap,
  },
  fill: { alignSelf: "stretch", flex: 1 },
  // The whole strip takes the press, whether the scale or the address shows.
  strip: { flex: 1, alignItems: "center", justifyContent: "center" },
  place: { maxWidth: "100%" },
  title: { alignItems: "center", gap: haloGap(8) },
  // The row is bounded, because a page lays out a row that does not wrap at
  // its content's width, and a long address would run over the buttons.
  search: {
    flexDirection: "row",
    alignItems: "center",
    gap: haloGap(8),
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
