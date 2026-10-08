// @pxl/web/app/store/pointer.ts

/**
 * A pointer on the world, sent to the kernel. A touch that starts there puts
 * the keyboard away, and while the dock is for typing it puts the typing away
 * instead of tapping: its lift ends the gesture with no tap, and its drag
 * still moves the world. Typing, the pointer is off the world, so nothing
 * hovers there and no tool shows what a tap would do.
 */

import { Keyboard } from "react-native";

import type { f64, i32 } from "@xpute/core/abi/word.ts";
import { PointerKind, PointerPhase } from "@/kernel/abi/cmd.spec.ts";
import { kernel_pointer } from "@/kernel/control/input.ts";
import { DockMode, useUiStore } from "@/app/store/ui.ts";

/** The pointers spent putting the typing away. */
const spent = new Set<i32>();
/** The pointers pressed, whose moves drag rather than hover. */
const pressed = new Set<i32>();

useUiStore.subscribe((state, before) => {
  if (state.dockMode === DockMode.Message && before.dockMode !== DockMode.Message) kernel_pointer(PointerPhase.Out, PointerKind.Mouse, 0, 0, 0, 0);
});

export function worldPointer(phase: PointerPhase, kind: PointerKind, id: i32, button: i32, x: f64, y: f64): void {
  const { dockMode, setDockMode } = useUiStore.getState();
  const typing = dockMode === DockMode.Message;
  switch (phase) {
    case PointerPhase.Down:
      pressed.add(id);
      Keyboard.dismiss();
      if (typing) {
        spent.add(id);
        setDockMode(DockMode.Palette);
      }
      break;
    case PointerPhase.Move:
      if (typing && !pressed.has(id)) return;
      break;
    case PointerPhase.Up:
    case PointerPhase.Cancel:
      pressed.delete(id);
      if (spent.delete(id)) phase = PointerPhase.Cancel;
      break;
  }
  kernel_pointer(phase, kind, id, button, x, y);
}
