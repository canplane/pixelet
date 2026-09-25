// @pxl/web/feature/map/ui/overlay/top-overlays.tsx

import { Search } from "lucide-react-native";
import { useTranslation } from "react-i18next";
import { Pressable, StyleSheet, View } from "react-native";

import { useViewportTop } from "@platform/visual-viewport.ts";
import { useAppContext } from "@/app/provider/context.ts";
import { alpha, chrome, color } from "@/shared/ui/theme.ts";
import { CurrentAddress } from "./current-address.tsx";
import { useReverseGeocode } from "./use-reverse-geocode.ts";
import { SensorStatus } from "./sensor-status.tsx";
import { DummyChatText } from "./dummy-chat-text.tsx";
import { MapCompass } from "../control/compass.tsx";
import { MapScaleBar } from "../control/scale-bar.tsx";
import { TrackingButton } from "@/feature/map/ui/control/tracking-button.tsx";
import { SettingsTriggerButton } from "@/feature/setting/trigger-button.tsx";

export function TopOverlays() {
  const { ui } = useAppContext();
  const { t } = useTranslation();
  const top = chrome.inset + useViewportTop();

  const addressLabel = useReverseGeocode();

  return (
    <>
      <View style={[styles.left, { top }]}>
        <SettingsTriggerButton />
        {__IS_DEV__ && <DummyChatText />}
      </View>

      <View style={[styles.right, { top }]}>
        {
          /* Solid, not translucent — same as mode-bar.tsx's own chrome
          (its tool strip is a flat bg-neutral-900, not an alpha'd one), and
          matches the round controls' own solid cream. */
        }
        <View style={styles.pill}>
          {
            /* Opens forward search — the current-address label and "where
            do I want to go" share the same mental slot on a map, so this
            is the one trigger for both rather than a separate
            always-visible search bar. A sibling of SensorStatus, which is
            its own button, rather than a wrapper around the whole pill. */
          }
          <Pressable onPress={ui.openSearchModal} accessibilityRole="button" accessibilityLabel={t("map.search.trigger_aria_label")} style={styles.search}>
            {
              /* Always visible, unlike the address text (CurrentAddress
              renders nothing until a label resolves) — without this the
              whole trigger would be an invisible tap target on first
              load or over unmapped water. SensorStatus's icon size, the
              glyphs beside it in this same pill. */
            }
            <Search size={14} color={alpha(color.ink, 0.5)} />
            <CurrentAddress label={addressLabel} />
          </Pressable>
          <SensorStatus />
        </View>

        <View style={styles.controls}>
          <MapScaleBar />

          <View style={styles.stack}>
            <TrackingButton />
            <MapCompass />
          </View>
        </View>
      </View>
    </>
  );
}

const styles = StyleSheet.create({
  // A layer takes no touches itself, only what stands on it: the map between
  // the controls is still the map.
  left: { position: "absolute", left: chrome.inset, zIndex: 50, alignItems: "flex-start", gap: 12, pointerEvents: "box-none" },
  right: { position: "absolute", right: chrome.inset, zIndex: 50, alignItems: "flex-end", gap: 16, pointerEvents: "box-none" },
  pill: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
    height: 24,
    paddingHorizontal: 12,
    borderRadius: 12,
    backgroundColor: color.cream,
  },
  search: { flexDirection: "row", alignItems: "center", gap: 6, minWidth: 0, flexShrink: 1 },
  controls: { flexDirection: "row", alignItems: "flex-start", gap: chrome.controlGap },
  stack: { alignItems: "center", gap: chrome.controlGap },
});
