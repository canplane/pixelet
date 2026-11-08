// @pxl/client/ui/world/view.tsx

import type { ReactNode } from "react";
import { StyleSheet, View } from "react-native";

import { AboveKeyboard } from "@/ui/component/above-keyboard.tsx";
import { type InputDriver } from "@/service/platform/types.ts";
import * as surface from "@/service/platform/surface.tsx";
import { useSafeArea } from "@/service/platform/safe-area.ts";
import { useVisualViewport } from "@/service/platform/viewport.ts";
import { POINTER_KIND, POINTER_PHASE } from "@/service/core/input.ts";
import { surface_driver } from "@/service/core/surface.ts";
import { keyDown, keyUp, releaseKeys } from "./keyboard.ts";
import { releasePointers, worldPointer } from "./pointer.ts";
import { Corner } from "./corner.tsx";
import { bottomRest } from "@/ui/component/bottom-rest.ts";
import { Toaster } from "@/ui/world/toast/stack.tsx";
import { chrome, color, layer } from "@/ui/component/theme.ts";
import { KEEPS_KEYBOARD } from "@/ui/component/keyboard.tsx";

import { useChromeTop } from "./chrome.ts";
import { BusyDim } from "./busy-dim.tsx";
import { Dock } from "./dock/dock.tsx";
import { TopOverlays } from "@/ui/world/top/top.tsx";
import { DOCK_HEIGHT } from "./dock/card.tsx";
import { ToastController } from "@/ui/world/toast/controller.tsx";
import { useCore } from "@/ui/watch.ts";
import * as input from "@/service/core/input.ts";
import * as scene from "@/service/core/scene.ts";

const TOAST_LIFT_PX = chrome.inset + 20;

const PANEL_GAP_PX = 12;

const WORLD = surface_driver(scene.draw_world);

const INPUT: InputDriver = {
  open: input.open,
  pointer: (phase, kind, id, button, x, y) =>
    worldPointer(POINTER_PHASE[phase], POINTER_KIND[kind], id, button, x, y),
  wheel: input.wheel,
  keyDown,
  keyUp,
  releaseKeys,
  releasePointers,
  requestFrame: input.frame,
};

/** A component of its own, so that a change of the marker under the pointer
 * re-renders only the surface. */
function World() {
  const overMarker = useCore((s) => s.over_marker);
  return (
    <surface.World surface={WORLD} input={INPUT} overMarker={overMarker} />
  );
}

function BetweenChrome({ children }: { children: ReactNode }) {
  const top = useChromeTop() + chrome.control + PANEL_GAP_PX;
  const bottom =
    bottomRest(useSafeArea(), chrome.inset) + DOCK_HEIGHT + PANEL_GAP_PX;
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

      {/* The world puts the keyboard away itself (`worldPointer`), so the
          shared dismissal leaves its touches alone. */}
      <View {...KEEPS_KEYBOARD} style={StyleSheet.absoluteFill}>
        <World />
      </View>

      {/* The toasts lie under the chrome, so that the size popover covers
          them. */}
      <AboveKeyboard inset={chrome.inset} lift={DOCK_HEIGHT + TOAST_LIFT_PX}>
        <Toaster />
      </AboveKeyboard>

      <Overlays />
    </View>
  );
}

const styles = StyleSheet.create({
  root: { overflow: "hidden", backgroundColor: color.black },
  between: {
    position: "absolute",
    left: chrome.inset,
    zIndex: layer.debug,
    alignItems: "flex-start",
    pointerEvents: "box-none",
  },
  // The dock's bar is as wide as this box, so the box needs a definite width
  // for the bar to grow into.
  bottom: { width: "100%", minWidth: 0, alignItems: "center" },
});
