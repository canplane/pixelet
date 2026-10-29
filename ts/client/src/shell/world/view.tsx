// @pxl/client/shell/world/view.tsx

import type { ReactNode } from "react";
import { StyleSheet, View } from "react-native";

import { AboveKeyboard } from "@/shell/ui/above-keyboard.tsx";
import { bracket, type InputDriver } from "@/bracket.tsx";
import {
  kernel_frame,
  kernel_open,
  kernel_rect,
  kernel_visible,
  kernel_wheel,
  POINTER_KIND,
  POINTER_PHASE,
} from "@/abi/input.ts";
import { surface_role } from "@/abi/gpu.spec.ts";
import { surface_driver } from "@/abi/surface.ts";
import { keyDown, keyUp, releaseKeys } from "./keyboard.ts";
import { releasePointers, worldPointer } from "./pointer.ts";
import { Corner } from "./corner.tsx";
import { bottomRest } from "@/shell/ui/bottom-rest.ts";
import { Toaster } from "@/shell/world/toast/stack.tsx";
import { chrome, layer } from "@/shell/ui/theme.ts";
import { KEEPS_KEYBOARD } from "@/shell/ui/keyboard.tsx";

import { useChromeTop } from "./chrome.ts";
import { BusyDim } from "./busy-dim.tsx";
import { Dock } from "./dock/dock.tsx";
import { TopOverlays } from "@/shell/world/top/top.tsx";
import { DOCK_HEIGHT } from "./dock/card.tsx";
import { ToastController } from "@/shell/world/toast/controller.tsx";
import { useKernel } from "@/shell/store/watch.ts";

const TOAST_LIFT_PX = chrome.inset + 20;

const PANEL_GAP_PX = 12;

const WORLD = surface_driver(surface_role.WORLD);

/** The place's input as the world takes it: pointers and keys through the
 * shell's own stores first (`worldPointer`, `keyDown`), the box, the wheel and
 * the visibility to the kernel, and a moved box's turn to the strobe. */
const INPUT: InputDriver = {
  open: kernel_open,
  rect: kernel_rect,
  pointer: (phase, kind, id, button, x, y) =>
    worldPointer(POINTER_PHASE[phase], POINTER_KIND[kind], id, button, x, y),
  wheel: kernel_wheel,
  keyDown,
  keyUp,
  releaseKeys,
  releasePointers,
  visible: kernel_visible,
  requestFrame: kernel_frame,
};

/** Alone, so a marker under the pointer re-renders only the surface. */
function World() {
  const overMarker = useKernel((s) => s.over_marker);
  return (
    <bracket.surface.World
      surface={WORLD}
      input={INPUT}
      overMarker={overMarker}
    />
  );
}

/** The room a panel may fill between the settings button and the dock. */
function BetweenChrome({ children }: { children: ReactNode }) {
  const top = useChromeTop() + chrome.control + PANEL_GAP_PX;
  const bottom =
    bottomRest(bracket.view.useSafeArea(), chrome.inset) +
    DOCK_HEIGHT +
    PANEL_GAP_PX;
  return <View style={[styles.between, { top, bottom }]}>{children}</View>;
}

function Overlays() {
  bracket.view.useVisualViewport();

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

      {
        /* Under the chrome: the size popover is meant to cover the toasts. */
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
