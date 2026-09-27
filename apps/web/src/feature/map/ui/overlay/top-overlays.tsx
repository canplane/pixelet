// @pxl/web/feature/map/ui/overlay/top-overlays.tsx

import { Search } from "lucide-react-native";
import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, useWindowDimensions, View } from "react-native";
import { Defs, RadialGradient, Rect, Stop, Svg } from "react-native-svg";

import { useViewportTop } from "@platform/visual-viewport.ts";
import { useAppContext } from "@/app/provider/context.ts";
import { useKernel } from "@/kernel/shell.ts";
import { Presence } from "@/shared/ui/motion/presence.tsx";
import { LEAVE, SETTLE } from "@/shared/ui/motion/transition.ts";
import { OutlinedIcon } from "@/shared/ui/outlined-icon.tsx";
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
  const scrimWidth = Math.min(width - 2 * chrome.inset, SCRIM_WIDTH);
  const scrimDepth = top + chrome.control + SCRIM_TAIL;

  return (
    <>
      <View style={[styles.scrim, { top: -scrimDepth, left: (width - scrimWidth) / 2, width: scrimWidth, height: 2 * scrimDepth }]}>
        <Svg width="100%" height="100%">
          <Defs>
            <RadialGradient id="scrim" cx="50%" cy="50%" r="50%">
              {SCRIM_STOPS.map(([at, opacity]) => <Stop key={at} offset={at} stopColor={color.black} stopOpacity={opacity} />)}
            </RadialGradient>
          </Defs>
          <Rect width="100%" height="100%" fill="url(#scrim)" />
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
              <OutlinedIcon icon={Search} size={14} color={color.white} />
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

/** The dark the title hangs in: half an ellipse from the middle of the
 * screen's top edge, fading down past the controls and out to either side.
 * What lets white read over a bright ground, as a photo's caption does,
 * without blurring the map, drawing an edge across it, or darkening the
 * width of a wide screen where nothing is. The ellipse's upper half is off
 * the screen. It eases out rather than falling in a line, whose end the eye
 * reads as an edge: flat at both ends, smoothstep's curve, sampled into
 * stops. */
const SCRIM_OPACITY = 0.45;
const SCRIM_WIDTH = 560;
const SCRIM_TAIL = 32;
const SCRIM_STOPS = Array.from({ length: 9 }, (_, i) => {
  const t = i / 8;
  return [t, SCRIM_OPACITY * (1 - t * t * (3 - 2 * t))] as const;
});

const styles = StyleSheet.create({
  scrim: { position: "absolute", zIndex: 49, pointerEvents: "none" },
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
  title: { alignItems: "center", gap: 6 },
  // Held to the title's width: centered in a column, a page lays a row out
  // at its content's width however wide, and a line that will not wrap is
  // all content — the address ran over the buttons either side rather than
  // ending in an ellipsis, where a device's layout held it.
  search: { flexDirection: "row", alignItems: "center", gap: 6, minWidth: 0, maxWidth: "100%", flexShrink: 1 },
  // Where the address stood: the scale takes its place while the zoom moves.
  scale: { position: "absolute", top: 0, bottom: 0, left: 0, right: 0, alignItems: "center", justifyContent: "center", pointerEvents: "box-none" },
});
