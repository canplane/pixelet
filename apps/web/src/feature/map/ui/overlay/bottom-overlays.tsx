// @pxl/web/feature/map/ui/overlay/bottom-overlays.tsx

import { StyleSheet } from "react-native";

import { AboveKeyboard } from "@platform/above-keyboard.tsx";
import { chrome } from "@/shared/ui/theme.ts";
import { BusyDim } from "@/feature/map/ui/busy-dim.tsx";
import { ModeBar } from "./mode-bar.tsx";

export function BottomOverlays() {
  return (
    <AboveKeyboard inset={chrome.inset}>
      <BusyDim style={styles.row}>
        <ModeBar />
      </BusyDim>
    </AboveKeyboard>
  );
}

const styles = StyleSheet.create({
  // ModeBar's width chain needs a real ancestor width to grow into.
  row: { width: "100%", minWidth: 0, alignItems: "center" },
});
