// @pxl/web/feature/map/ui/overlay/top-overlays.tsx

import { Search } from "lucide-react-native";
import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, View } from "react-native";

import { useViewportTop } from "@platform/visual-viewport.ts";
import { useAppContext } from "@/app/provider/context.ts";
import { Presence } from "@/shared/ui/motion/presence.tsx";
import { LEAVE, SETTLE } from "@/shared/ui/motion/transition.ts";
import { OutlinedIcon } from "@/shared/ui/outlined-icon.tsx";
import { chrome, color } from "@/shared/ui/theme.ts";
import { CurrentAddress } from "./current-address.tsx";
import { useReverseGeocode } from "./use-reverse-geocode.ts";
import { SensorStatus } from "./sensor-status.tsx";
import { DummyChatText } from "./dummy-chat-text.tsx";
import { MapCompass } from "../control/compass.tsx";
import { MapScaleBar, useZooming } from "../control/scale-bar.tsx";
import { TrackingButton } from "@/feature/map/ui/control/tracking-button.tsx";
import { BusyDim } from "@/feature/map/ui/busy-dim.tsx";
import { SettingsTriggerButton } from "@/feature/setting/trigger-button.tsx";

export function TopOverlays() {
  const { ui } = useAppContext();
  const { t } = useTranslation();
  const top = chrome.inset + useViewportTop();

  const addressLabel = useReverseGeocode();
  const zooming = useZooming();

  return (
    <>
      <BusyDim style={[styles.left, { top }]}>
        <SettingsTriggerButton />
        {__IS_DEV__ && <DummyChatText />}
      </BusyDim>

      <BusyDim style={[styles.center, { top }]}>
        <Presence show={!zooming} from={{ opacity: 0 }} exit={{ opacity: 0 }} transition={SETTLE} exitTransition={LEAVE} style={styles.place}>
          {
            /* Opens forward search — the current-address label and "where
            do I want to go" share the same mental slot on a map, so this
            is the one trigger for both rather than a separate
            always-visible search bar. A sibling of SensorStatus, which is
            its own button, rather than a wrapper around both. */
          }
          <Pressable onPress={ui.openSearchModal} accessibilityRole="button" accessibilityLabel={t("map.search.trigger_aria_label")} style={styles.search}>
            {
              /* Always visible, unlike the address text (CurrentAddress
              renders nothing until a label resolves) — without this the
              whole trigger would be an invisible tap target on first
              load or over unmapped water. SensorStatus's icon size. */
            }
            <OutlinedIcon icon={Search} size={14} color={color.white} />
            <CurrentAddress label={addressLabel} />
          </Pressable>
          <SensorStatus />
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

const styles = StyleSheet.create({
  // A layer takes no touches itself, only what stands on it (BusyDim): the
  // map between the controls is still the map.
  left: { position: "absolute", left: chrome.inset, zIndex: 50, alignItems: "flex-start", gap: 12 },
  center: {
    position: "absolute",
    left: 0,
    right: 0,
    paddingHorizontal: chrome.inset + chrome.control + chrome.controlGap,
    zIndex: 50,
    alignItems: "center",
    gap: chrome.controlGap,
  },
  right: { position: "absolute", right: chrome.inset, zIndex: 50, alignItems: "center", gap: chrome.controlGap },
  place: { maxWidth: "100%", alignItems: "center", gap: 6 },
  search: { flexDirection: "row", alignItems: "center", gap: 6, minWidth: 0, flexShrink: 1 },
  // Where the address stood: the scale takes its place while the zoom moves.
  scale: { position: "absolute", top: 0, left: 0, right: 0, alignItems: "center", pointerEvents: "box-none" },
});
