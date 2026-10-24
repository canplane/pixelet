// @pxl/client/shell/world/overlay/top.tsx

import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, View } from "react-native";
import { Defs, LinearGradient, Mask, Rect, Stop, Svg } from "react-native-svg";

import { useWindowSize } from "@/bracket.tsx";
import { openModal } from "@/shell/modal.tsx";
import { useKernel } from "@/abi/shell.ts";
import { Motion } from "@/shell/ui/motion/motion.tsx";
import { Presence } from "@/shell/ui/motion/presence.tsx";
import { LEAVE, SETTLE, SNAP } from "@/shell/ui/motion/transition.ts";
import { useVeil } from "@/shell/ui/motion/veil.ts";
import { haloGap, HaloIcon } from "@/shell/ui/halo.tsx";
import { chrome, color, layer } from "@/shell/ui/theme.ts";
import { CurrentAddress } from "./current-address.tsx";
import { SensorStatus } from "./sensor-status.tsx";
import { MapScaleBar } from "../control/scale-bar.tsx";
import { LocationButtons } from "../control/location-buttons.tsx";
import { ViewButtons } from "../control/view-buttons.tsx";
import { BusyDim } from "../busy-dim.tsx";
import { useChromeTop } from "../chrome.ts";
import { CircularButton } from "@/shell/ui/circular-button.tsx";

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
              {SCRIM_EASE.map(([at, k]) => <Stop key={at} offset={at} stopColor={color.black} stopOpacity={SCRIM_OPACITY * (1 - k)} />)}
            </LinearGradient>
            <LinearGradient id="scrim-across" gradientUnits="userSpaceOnUse" x1={from} y1={0} x2={width - from} y2={0}>
              {[...SCRIM_EASE.map(([at, k]) => [ramp * at, k] as const), ...[...SCRIM_EASE].reverse().map(([at, k]) => [1 - ramp * at, k] as const)].map(([at, k], i) => (
                <Stop key={i} offset={at} stopColor={color.white} stopOpacity={k} />
              ))}
            </LinearGradient>
            <Mask id="scrim-mask">
              <Rect width="100%" height="100%" fill="url(#scrim-across)" />
            </Mask>
          </Defs>
          <Rect width="100%" height="100%" fill="url(#scrim-down)" mask="url(#scrim-mask)" />
        </Svg>
      </View>

      <BusyDim style={[styles.left, { top }]}>
        <CircularButton glyph="line.3.horizontal" onPress={() => openModal("settings")} label={t("settings.trigger_aria_label")} />
      </BusyDim>

      <BusyDim style={[styles.center, { top }]}>
        <SearchStrip />
      </BusyDim>

      <BusyDim style={[styles.right, { top }]}>
        <LocationButtons />
        <ViewButtons />
      </BusyDim>
    </>
  );
}

/** Inside the dim, so the address and the scale fade with the buttons. */
function SearchStrip() {
  const { t } = useTranslation();
  const zooming = useKernel((s) => s.zooming);
  const veil = useVeil(1, SNAP);
  return (
    <Motion {...veil.motion} style={styles.fill}>
      <Pressable onPress={() => openModal("search")} accessibilityRole="button" accessibilityLabel={t("world.search.trigger_aria_label")} style={styles.strip}>
        <Presence show={!zooming} from={{ opacity: 0 }} exit={{ opacity: 0 }} transition={SETTLE} exitTransition={LEAVE} style={styles.place}>
          <View style={styles.title}>
            <View style={styles.search}>
              <HaloIcon glyph="magnifyingglass" size={12} weight="semibold" color={color.white} />
              <CurrentAddress />
            </View>
            <SensorStatus />
          </View>
        </Presence>
        <View style={styles.scale}>
          <MapScaleBar show={zooming} />
        </View>
      </Pressable>
    </Motion>
  );
}

/** A scrim so white reads over a bright map without a blur: flat over
 * SCRIM_WIDTH, then smoothstep fades, since a straight fade's end reads as an edge. */
const SCRIM_OPACITY = 0.45;
const SCRIM_WIDTH = 560;
const SCRIM_FADE = 240;
const SCRIM_TAIL = 32;
const SCRIM_EASE = Array.from({ length: 9 }, (_, i) => {
  const t = i / 8;
  return [t, t * t * (3 - 2 * t)] as const;
});

const styles = StyleSheet.create({
  scrim: { position: "absolute", top: 0, left: 0, right: 0, zIndex: layer.scrim, pointerEvents: "none" },
  left: { position: "absolute", left: chrome.inset, zIndex: layer.chrome, alignItems: "flex-start", gap: 12 },
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
  right: { position: "absolute", right: chrome.inset, zIndex: layer.chrome, alignItems: "center", gap: chrome.controlGap },
  fill: { alignSelf: "stretch", flex: 1 },
  // The whole strip, scale shown or not.
  strip: { flex: 1, alignItems: "center", justifyContent: "center" },
  place: { maxWidth: "100%" },
  title: { alignItems: "center", gap: haloGap(8) },
  // Bounded: a page lays a no-wrap row at content width, and the address overran the buttons.
  search: { flexDirection: "row", alignItems: "center", gap: haloGap(8), minWidth: 0, maxWidth: "100%", flexShrink: 1 },
  scale: { ...StyleSheet.absoluteFill, alignItems: "center", justifyContent: "center", pointerEvents: "none" },
});
