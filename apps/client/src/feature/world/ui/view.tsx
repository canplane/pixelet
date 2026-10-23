// @pxl/client/feature/world/ui/view.tsx

import type { ReactNode } from "react";
import { StyleSheet, View } from "react-native";

import { AboveKeyboard } from "@platform/above-keyboard.tsx";
import { useSafeArea } from "@platform/safe-area.ts";
import { useVisualViewport } from "@platform/viewport.ts";
import { WorldSurface } from "@platform/surface.tsx";
import { Corner } from "./corner.tsx";
import { bottomRest } from "@/shared/ui/bottom-rest.ts";
import { Toaster } from "@/shared/ui/toast/stack.tsx";
import { chrome, layer } from "@/shared/ui/theme.ts";
import { KEEPS_KEYBOARD } from "@/shared/ui/keyboard.tsx";

import { useChromeTop } from "./chrome.ts";
import { BusyDim } from "./busy-dim.tsx";
import { Dock } from "./overlay/dock.tsx";
import { TopOverlays } from "./overlay/top.tsx";
import { DOCK_HEIGHT } from "./overlay/dock.tsx";
import { ToastController } from "./toast-controller.tsx";

const TOAST_LIFT_PX = chrome.inset + 20;

const PANEL_GAP_PX = 12;

/** The room a panel may fill between the settings button and the dock. */
function BetweenChrome({ children }: { children: ReactNode }) {
  const top = useChromeTop() + chrome.control + PANEL_GAP_PX;
  const bottom = bottomRest(useSafeArea(), chrome.inset) + DOCK_HEIGHT + PANEL_GAP_PX;
  return <View style={[styles.between, { top, bottom }]}>{children}</View>;
}

function Overlays() {
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

export function WorldView() {
  return (
    <View style={[StyleSheet.absoluteFill, styles.root]}>
      <ToastController />
      <BetweenChrome>
        <Corner />
      </BetweenChrome>

      {/* The world's own pointer puts the keyboard away (`worldPointer`). */}
      <View {...KEEPS_KEYBOARD} style={StyleSheet.absoluteFill}>
        <WorldSurface />
      </View>

      {
        /* Under the chrome: the size flyout is meant to cover the toasts. */
      }
      <AboveKeyboard inset={chrome.inset} lift={DOCK_HEIGHT + TOAST_LIFT_PX}>
        <Toaster />
      </AboveKeyboard>

      <Overlays />
    </View>
  );
}

const styles = StyleSheet.create({
  root: { overflow: "hidden", backgroundColor: "black" },
  between: { position: "absolute", left: chrome.inset, zIndex: layer.debug, alignItems: "flex-start", pointerEvents: "box-none" },
  // The dock's width chain needs a real ancestor width to grow into.
  bottom: { width: "100%", minWidth: 0, alignItems: "center" },
});
