// @pxl/web/feature/map/ui/overlay/all.tsx

import { StyleSheet } from "react-native";

import { AboveKeyboard } from "@platform/above-keyboard.tsx";
import { useVisualViewport } from "@platform/viewport.ts";
import { BusyDim } from "@/feature/map/ui/busy-dim.tsx";
import { chrome } from "@/shared/ui/theme.ts";
import { Dock } from "./dock.tsx";
import { TopOverlays } from "./top.tsx";

export function Overlays() {
  useVisualViewport();

  return (
    <>
      <TopOverlays />
      <AboveKeyboard inset={chrome.inset}>
        <BusyDim style={styles.bottom}>
          <Dock />
        </BusyDim>
      </AboveKeyboard>
    </>
  );
}

const styles = StyleSheet.create({
  // The dock's width chain needs a real ancestor width to grow into.
  bottom: { width: "100%", minWidth: 0, alignItems: "center" },
});
