// @pxl/web/app/store/pointer.ts

/**
 * A pointer on the world, sent to the kernel. A touch that starts there puts
 * the keyboard away, and while the dock is for typing it puts the typing away
 * instead of tapping: its lift ends the gesture with no tap, and its drag
 * still moves the world.
 */

import { Keyboard } from "react-native";

import type { f64, i32 } from "@xpute/core/abi/word.ts";
import { type PointerKind, PointerPhase } from "@/kernel/abi/cmd.spec.ts";
import { kernel_pointer } from "@/kernel/control/input.ts";
import { DockMode, useUiStore } from "@/app/store/ui.ts";

/** The pointers spent putting the typing away. */
const spent = new Set<i32>();

export function worldPointer(phase: PointerPhase, kind: PointerKind, id: i32, button: i32, x: f64, y: f64): void {
  if (phase === PointerPhase.Down) {
    Keyboard.dismiss();
    const { dockMode, setDockMode } = useUiStore.getState();
    if (dockMode === DockMode.Message) {
      spent.add(id);
      setDockMode(DockMode.Palette);
    }
  } else if ((phase === PointerPhase.Up || phase === PointerPhase.Cancel) && spent.delete(id)) {
    phase = PointerPhase.Cancel;
  }
  kernel_pointer(phase, kind, id, button, x, y);
}
