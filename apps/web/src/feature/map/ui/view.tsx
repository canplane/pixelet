// @pxl/web/feature/map/ui/view.tsx

import type { ReactNode } from "react";
import { StyleSheet, View } from "react-native";

import { AboveKeyboard } from "@platform/above-keyboard.tsx";
import { useSafeArea } from "@platform/safe-area.ts";
import { useViewportTop } from "@platform/viewport.ts";
import { WorldSurface } from "@platform/surface.tsx";
import { DebugOverlay } from "@/feature/debug/overlay.tsx";
import { bottomRest } from "@/shared/ui/bottom-rest.ts";
import { Toaster } from "@/shared/ui/toast/stack.tsx";
import { chrome } from "@/shared/ui/theme.ts";

import { Overlays } from "./overlay/all.tsx";
import { ToastController } from "./toast-controller.tsx";

const TOAST_LIFT_PX = chrome.inset + 20;

const PANEL_GAP_PX = 12;

/** The room a panel may fill between the settings button and the dock. */
function BetweenChrome({ children }: { children: ReactNode }) {
  const area = useSafeArea();
  const top = chrome.inset + area.top + useViewportTop() + chrome.control + PANEL_GAP_PX;
  const bottom = bottomRest(area, chrome.inset) + chrome.control + PANEL_GAP_PX;
  return <View style={[styles.between, { top, bottom }]}>{children}</View>;
}

export function MapView() {
  return (
    <View style={[StyleSheet.absoluteFill, styles.root]}>
      <ToastController />
      <BetweenChrome>
        <DebugOverlay />
      </BetweenChrome>

      <View style={StyleSheet.absoluteFill}>
        <WorldSurface />
      </View>

      {
        /* Under the chrome: the size flyout is meant to cover the toasts. */
      }
      <AboveKeyboard inset={chrome.inset} lift={chrome.control + TOAST_LIFT_PX}>
        <Toaster />
      </AboveKeyboard>

      <Overlays />
    </View>
  );
}

const styles = StyleSheet.create({
  root: { overflow: "hidden", backgroundColor: "black" },
  between: { position: "absolute", left: chrome.inset, zIndex: 50, alignItems: "flex-start", pointerEvents: "box-none" },
});
