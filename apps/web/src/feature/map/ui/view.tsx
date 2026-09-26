// @pxl/web/feature/map/ui/view.tsx

/**
 * The map as the page shows it: the world's surface, handed to the kernel,
 * and the chrome on top. The camera and its input, the world, the
 * environment, the fetches that follow the view and the drawing are the
 * kernel's turn (kernel/kernel.ts); this lays out the surface and the chrome.
 */

import { StyleSheet, View } from "react-native";

import { AboveKeyboard } from "@platform/above-keyboard.tsx";
import { WorldSurface } from "@platform/surface.tsx";
import { LedgerOverlay } from "@/feature/dev/ledger/overlay.tsx";
import { Toaster } from "@/shared/ui/toast/toaster.tsx";
import { chrome } from "@/shared/ui/theme.ts";

import { Overlays } from "./overlay/overlays.tsx";
import { MODE_BAR_HEIGHT_PX } from "./overlay/mode-bar.tsx";
import { ToastController } from "./toast-controller.tsx";

/** How far the toasts stand over the mode bar: a gap as wide as the
 * chrome's inset, and a lift past it. */
const TOAST_LIFT_PX = chrome.inset + 20;

export function MapView() {
  return (
    <View style={[StyleSheet.absoluteFill, styles.root]}>
      <ToastController />
      <LedgerOverlay />

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
});
