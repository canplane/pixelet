// @pxl/web/feature/map/ui/overlay/bottom.tsx

import { StyleSheet } from "react-native";

import { AboveKeyboard } from "@platform/above-keyboard.tsx";
import { chrome } from "@/shared/ui/theme.ts";
import { BusyDim } from "@/feature/map/ui/busy-dim.tsx";
import { MapCompass } from "../control/compass.tsx";
import { PersonButton } from "../control/person-button.tsx";
import { TrackingButton } from "../control/tracking-button.tsx";
import { MODE_BAR_HEIGHT_PX, ModeBar } from "./mode-bar.tsx";

/** The view's own controls float at the bottom right over the mode bar, as
 * Google Earth's do. */
export function BottomOverlays() {
  return (
    <AboveKeyboard inset={chrome.inset}>
      <BusyDim style={styles.side}>
        <MapCompass />
        <PersonButton />
        <TrackingButton />
      </BusyDim>
      <BusyDim style={styles.row}>
        <ModeBar />
      </BusyDim>
    </AboveKeyboard>
  );
}

const styles = StyleSheet.create({
  side: { position: "absolute", right: 0, bottom: MODE_BAR_HEIGHT_PX + chrome.controlGap * 2, alignItems: "center", gap: chrome.controlGap },
  // ModeBar's width chain needs a real ancestor width to grow into.
  row: { width: "100%", minWidth: 0, alignItems: "center" },
});
