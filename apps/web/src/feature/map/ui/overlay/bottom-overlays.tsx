// @pxl/web/feature/map/ui/overlay/bottom-overlays.tsx

import { StyleSheet, View } from "react-native";

import { AboveKeyboard } from "@platform/above-keyboard.tsx";
import { chrome } from "@/shared/ui/theme.ts";
import { ModeBar } from "./mode-bar.tsx";

export function BottomOverlays() {
  return (
    <AboveKeyboard inset={chrome.inset}>
      <View style={styles.row}>
        <ModeBar />
      </View>
    </AboveKeyboard>
  );
}

const styles = StyleSheet.create({
  // ModeBar's own width chain needs a real ancestor width to resolve
  // against — without it, a growing child has nothing to grow into. The
  // row itself takes no touches, only the bar on it.
  row: { width: "100%", minWidth: 0, alignItems: "center", pointerEvents: "box-none" },
});
