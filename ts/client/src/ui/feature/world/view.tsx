// @pxl/client/ui/feature/world/view.tsx

import type { ReactNode } from "react";
import { StyleSheet, View } from "react-native";

import { AboveKeyboard } from "@/ui/shared/component/above-keyboard.tsx";
import { type InputDriver } from "@/service/platform/types.ts";
import * as surface from "@/service/platform/surface.tsx";
import { useSafeArea } from "@/service/platform/safe-area.ts";
import { useVisualViewport } from "@/service/platform/viewport.ts";
import { POINTER_KIND, POINTER_PHASE } from "@/service/core/input.ts";
import { surface_driver } from "@/service/core/surface.ts";
import { key_down, key_up, release_keys } from "./keyboard.ts";
import { release_pointers, world_pointer } from "./pointer.ts";
import { Corner } from "./corner.tsx";
import { bottom_rest } from "@/ui/shared/component/bottom-rest.ts";
import { Toaster } from "@/ui/feature/world/toast/stack.tsx";
import { CHROME, COLOR, LAYER } from "@/ui/shared/component/theme.ts";
import { KEEPS_KEYBOARD } from "@/ui/shared/component/keyboard.tsx";

import { useChromeTop } from "./chrome.ts";
import { BusyDim } from "./busy-dim.tsx";
import { Dock } from "./dock/dock.tsx";
import { TopOverlays } from "@/ui/feature/world/top/top.tsx";
import { DOCK_HEIGHT } from "./dock/card.tsx";
import { ToastController } from "@/ui/feature/world/toast/controller.tsx";
import { useCore } from "@/ui/shared/watch.ts";
import * as input from "@/service/core/input.ts";
import * as scene from "@/service/core/scene.ts";

const TOAST_LIFT_PX = CHROME.inset + 20;

const PANEL_GAP_PX = 12;

const WORLD = surface_driver(scene.draw_world);

const INPUT: InputDriver = {
  open: input.open,
  pointer: (phase, kind, id, button, x, y) =>
    world_pointer(POINTER_PHASE[phase], POINTER_KIND[kind], id, button, x, y),
  wheel: input.wheel,
  pinch: input.pinch,
  key_down,
  key_up,
  release_keys,
  release_pointers,
};

/** A component of its own, so that a change of the marker under the pointer
 * re-renders only the surface. */
function World() {
  const over_marker = useCore((s) => s.over_marker);
  return (
    <surface.World surface={WORLD} input={INPUT} overMarker={over_marker} />
  );
}

function BetweenChrome({ children }: { children: ReactNode }) {
  const top = useChromeTop() + CHROME.control + PANEL_GAP_PX;
  const bottom =
    bottom_rest(useSafeArea(), CHROME.inset) + DOCK_HEIGHT + PANEL_GAP_PX;
  return <View style={[STYLE.between, { top, bottom }]}>{children}</View>;
}

function Overlays() {
  useVisualViewport();

  return (
    <>
      <TopOverlays />
      <AboveKeyboard inset={CHROME.inset}>
        <BusyDim style={STYLE.bottom}>
          <Dock />
        </BusyDim>
      </AboveKeyboard>
    </>
  );
}

export function WorldView() {
  return (
    <View style={[StyleSheet.absoluteFill, STYLE.root]}>
      <ToastController />
      <BetweenChrome>
        <Corner />
      </BetweenChrome>

      {/* The world puts the keyboard away itself (`world_pointer`), so the
          shared dismissal leaves its touches alone. */}
      <View {...KEEPS_KEYBOARD} style={StyleSheet.absoluteFill}>
        <World />
      </View>

      {/* The toasts lie under the chrome, so that the size popover covers
          them. */}
      <AboveKeyboard inset={CHROME.inset} lift={DOCK_HEIGHT + TOAST_LIFT_PX}>
        <Toaster />
      </AboveKeyboard>

      <Overlays />
    </View>
  );
}

const STYLE = StyleSheet.create({
  root: { overflow: "hidden", backgroundColor: COLOR.black },
  between: {
    position: "absolute",
    left: CHROME.inset,
    zIndex: LAYER.debug,
    alignItems: "flex-start",
    pointerEvents: "box-none",
  },
  // The dock's bar is as wide as this box, so the box needs a definite width
  // for the bar to grow into.
  bottom: { width: "100%", minWidth: 0, alignItems: "center" },
});
