// @pxl/web/feature/map/ui/overlay/top-overlays.tsx

import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, useWindowDimensions, View } from "react-native";
import { Defs, LinearGradient, Mask, Rect, Stop, Svg } from "react-native-svg";

import { useViewportTop } from "@platform/viewport.ts";
import { useAppContext } from "@/app/provider/context.ts";
import { useKernel } from "@/kernel/shell.ts";
import { Presence } from "@/shared/ui/motion/presence.tsx";
import { LEAVE, SETTLE } from "@/shared/ui/motion/transition.ts";
import { haloGap, HaloIcon } from "@/shared/ui/halo.tsx";
import { chrome, color } from "@/shared/ui/theme.ts";
import { CurrentAddress } from "./current-address.tsx";
import { useReverseGeocode } from "./use-reverse-geocode.ts";
import { SensorStatus } from "./sensor-status.tsx";
import { DummyChatText } from "./dummy-chat-text.tsx";
import { MapCompass } from "../control/compass.tsx";
import { MapScaleBar } from "../control/scale-bar.tsx";
import { TrackingButton } from "@/feature/map/ui/control/tracking-button.tsx";
import { BusyDim } from "@/feature/map/ui/busy-dim.tsx";
import { SettingsTriggerButton } from "@/feature/setting/trigger-button.tsx";

export function TopOverlays() {
  const { ui } = useAppContext();
  const { t } = useTranslation();
  const top = chrome.inset + useViewportTop();

  const addressLabel = useReverseGeocode();
  const zooming = useKernel((s) => s.zooming);
  const { width } = useWindowDimensions();
  const scrimDepth = top + chrome.control + SCRIM_TAIL;
  // The fade across runs from `from` to the plateau's edge and back on the
  // other side. On a phone the plateau is the whole width, and the fades lie
  // off the screen.
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
              {[...SCRIM_EASE.map(([at, k]) => [ramp * at, k] as const), ...SCRIM_EASE.toReversed().map(([at, k]) => [1 - ramp * at, k] as const)].map(([at, k], i) => (
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
        <SettingsTriggerButton />
        {__IS_DEV__ && <DummyChatText />}
      </BusyDim>

      <BusyDim style={[styles.center, { top }]}>
        <Presence show={!zooming} from={{ opacity: 0 }} exit={{ opacity: 0 }} transition={SETTLE} exitTransition={LEAVE} style={styles.place}>
          {
            /* Opens forward search — the current-address label and "where
            do I want to go" share the same mental slot on a map, so the
            whole title, the sensors under it with it, is the one trigger
            for both rather than a separate always-visible search bar. */
          }
          <Pressable onPress={ui.openSearchModal} accessibilityRole="button" accessibilityLabel={t("map.search.trigger_aria_label")} style={styles.title}>
            <View style={styles.search}>
              <HaloIcon glyph="magnifyingglass" size={14} weight="semibold" color={color.white} />
              <CurrentAddress label={addressLabel} />
            </View>
            <SensorStatus />
          </Pressable>
        </Presence>
        <View style={styles.scale}>
          <MapScaleBar show={zooming} />
        </View>
      </BusyDim>

      <BusyDim style={[styles.right, { top }]}>
        <TrackingButton />
        <MapCompass />
      </BusyDim>
    </>
  );
}

/** The dark behind the top controls, so white text and glyphs read over a
 * bright ground without a blur or an edge across the map. It fades down from
 * the screen's top edge to past the controls, and across it is flat over at
 * most SCRIM_WIDTH about the center and fades out beyond: on a phone that is
 * the whole width, status bar included, as a system's top protection is; on
 * a wide screen it stays near the title instead of darkening empty map.
 * Every fade follows smoothstep, sampled into stops, since a straight fade's
 * end reads as an edge. */
const SCRIM_OPACITY = 0.45;
const SCRIM_WIDTH = 560;
const SCRIM_FADE = 240;
const SCRIM_TAIL = 32;
/** [position, smoothstep(position)] from 0 to 1. */
const SCRIM_EASE = Array.from({ length: 9 }, (_, i) => {
  const t = i / 8;
  return [t, t * t * (3 - 2 * t)] as const;
});

const styles = StyleSheet.create({
  scrim: { position: "absolute", top: 0, left: 0, right: 0, zIndex: 49, pointerEvents: "none" },
  // A layer takes no touches itself, only what stands on it (BusyDim): the
  // map between the controls is still the map.
  left: { position: "absolute", left: chrome.inset, zIndex: 50, alignItems: "flex-start", gap: 12 },
  center: {
    position: "absolute",
    left: 0,
    right: 0,
    paddingHorizontal: chrome.inset + chrome.control + chrome.controlGap,
    height: chrome.control,
    zIndex: 50,
    alignItems: "center",
    justifyContent: "center",
  },
  right: { position: "absolute", right: chrome.inset, zIndex: 50, alignItems: "center", gap: chrome.controlGap },
  place: { maxWidth: "100%" },
  title: { alignItems: "center", gap: haloGap(8) },
  // Held to the title's width: centered in a column, a page lays a row out
  // at its content's width however wide, and a line that will not wrap is
  // all content — the address ran over the buttons either side rather than
  // ending in an ellipsis, where a device's layout held it.
  search: { flexDirection: "row", alignItems: "center", gap: haloGap(8), minWidth: 0, maxWidth: "100%", flexShrink: 1 },
  // Where the address stood: the scale takes its place while the zoom moves.
  scale: { position: "absolute", top: 0, bottom: 0, left: 0, right: 0, alignItems: "center", justifyContent: "center", pointerEvents: "box-none" },
});
