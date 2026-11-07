// @pxl/client/ui/world/view.tsx

import type { ReactNode } from "react";
import { StyleSheet, View } from "react-native";

import { AboveKeyboard } from "@/ui/component/above-keyboard.tsx";
import { type InputDriver } from "@/service/types.ts";
import * as surface from "@/service/surface.tsx";
import { useSafeArea } from "@/service/safe-area.ts";
import { useVisualViewport } from "@/service/viewport.ts";
import { POINTER_KIND, POINTER_PHASE } from "@/port/input.ts";
import { surface_driver } from "@/port/surface.ts";
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
import { useCore } from "@/ui/store/watch.ts";
import * as input from "@/port/input.ts";
import * as scene from "@/port/scene.ts";

const TOAST_LIFT_PX = chrome.inset + 20;

const PANEL_GAP_PX = 12;

const WORLD = surface_driver(scene.draw_world);

/** The place's input as the world takes it: pointers and keys through the
 * UI's own stores first (`worldPointer`, `keyDown`), the box and the wheel
 * to the core, and a moved box's turn to the scheduler. */
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

/** Alone, so a marker under the pointer re-renders only the surface. */
function World() {
  const overMarker = useCore((s) => s.over_marker);
  return (
    <surface.World surface={WORLD} input={INPUT} overMarker={overMarker} />
  );
}

/** The room a panel may fill between the settings button and the dock. */
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

      {/* The world's own pointer puts the keyboard away (`worldPointer`). */}
      <View {...KEEPS_KEYBOARD} style={StyleSheet.absoluteFill}>
        <World />
      </View>

      {/* Under the chrome: the size popover is meant to cover the toasts. */}
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
  // The dock's width chain needs a real ancestor width to grow into.
  bottom: { width: "100%", minWidth: 0, alignItems: "center" },
});
