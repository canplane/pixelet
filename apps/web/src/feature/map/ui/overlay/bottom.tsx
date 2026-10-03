// @pxl/web/feature/map/ui/overlay/bottom.tsx

import { StyleSheet } from "react-native";

import { AboveKeyboard } from "@platform/above-keyboard.tsx";
import { chrome } from "@/shared/ui/theme.ts";
import { BusyDim } from "@/feature/map/ui/busy-dim.tsx";
import { Dock } from "./dock.tsx";

export function BottomOverlays() {
  return (
    <AboveKeyboard inset={chrome.inset}>
      <BusyDim style={styles.row}>
        <Dock />
      </BusyDim>
    </AboveKeyboard>
  );
}

const styles = StyleSheet.create({
  // The dock's width chain needs a real ancestor width to grow into.
  row: { width: "100%", minWidth: 0, alignItems: "center" },
});
