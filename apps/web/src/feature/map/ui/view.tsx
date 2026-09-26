// @pxl/web/feature/map/ui/view.tsx

/**
 * The map as the page shows it: the world's surface, handed to the kernel,
 * and the chrome on top. The camera and its input, the world, the
 * environment, the fetches that follow the view and the drawing are the
 * kernel's turn (kernel/kernel.ts); this lays out the surface and the chrome.
 */

import type { ReactNode } from "react";
import { StyleSheet, View } from "react-native";

import { AboveKeyboard } from "@platform/above-keyboard.tsx";
import { useSafeArea } from "@platform/safe-area.ts";
import { useViewportTop } from "@platform/visual-viewport.ts";
import { WorldSurface } from "@platform/surface.tsx";
import { LedgerOverlay } from "@/feature/debug/ledger/overlay.tsx";
import { Toaster } from "@/shared/ui/toast/toaster.tsx";
import { chrome } from "@/shared/ui/theme.ts";

import { Overlays } from "./overlay/overlays.tsx";
import { MODE_BAR_HEIGHT_PX } from "./overlay/mode-bar.tsx";
import { ToastController } from "./toast-controller.tsx";

/** How far the toasts stand over the mode bar: a gap as wide as the
 * chrome's inset, and a lift past it. */
const TOAST_LIFT_PX = chrome.inset + 20;

/** The gap a panel keeps from the chrome above and below it: the top
 * column's own. */
const PANEL_GAP_PX = 12;

/** The room between the settings button and the mode bar, down the left
 * side: what a panel laid over the map may fill and not pass, however long
 * it grows. The keyboard is not followed — the mode bar rides it, and a
 * panel is not what one types beside. */
function BetweenChrome({ children }: { children: ReactNode }) {
  const top = chrome.inset + useViewportTop() + chrome.control + PANEL_GAP_PX;
  const bottom = chrome.inset + useSafeArea().bottom + MODE_BAR_HEIGHT_PX + PANEL_GAP_PX;
  return <View style={[styles.between, { top, bottom }]}>{children}</View>;
}

export function MapView() {
  return (
    <View style={[StyleSheet.absoluteFill, styles.root]}>
      <ToastController />
      <BetweenChrome>
        <LedgerOverlay />
      </BetweenChrome>

      <View style={StyleSheet.absoluteFill}>
        <WorldSurface />
      </View>

      {
        /* Under the chrome, not over it: the toasts stand clear of the
        mode bar, and what the bar opens over them — the size flyout —
        is meant to cover them. */
      }
      <AboveKeyboard inset={chrome.inset} lift={MODE_BAR_HEIGHT_PX + TOAST_LIFT_PX}>
        <Toaster />
      </AboveKeyboard>

      <Overlays />
    </View>
  );
}

const styles = StyleSheet.create({
  root: { overflow: "hidden", backgroundColor: "black" },
  // Over the map's chrome, taking no touches but the panel's.
  between: { position: "absolute", left: chrome.inset, zIndex: 50, alignItems: "flex-start", pointerEvents: "box-none" },
});
