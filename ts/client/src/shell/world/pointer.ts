// @pxl/client/shell/world/pointer.ts

/**
 * A pointer on the world, sent to the kernel. A touch that starts there puts
 * the keyboard away, and while the dock is for typing it puts the typing away
 * instead of tapping: its lift ends the gesture with no tap, and its drag
 * still moves the world. Typing, the pointer is off the world, so nothing
 * hovers there and no tool shows what a tap would do.
 */

import type { f64, i32 } from "@xpute/kit/abi/word.ts";
import { PointerPhase, PointerType } from "@/abi/cmd.spec.ts";
import { kernel_pointer } from "@/abi/input.ts";
import { putKeyboardAway } from "@/shell/ui/keyboard.tsx";
import { DockMode, useDockStore } from "@/shell/store/dock.ts";

/** The pointers spent putting the typing away. */
const spent = new Set<i32>();
/** The pointers pressed, whose moves drag rather than hover, each with its kind. */
const pressed = new Map<i32, PointerType>();

useDockStore.subscribe((state, before) => {
  if (
    state.dockMode === DockMode.Message &&
    before.dockMode !== DockMode.Message
  )
    kernel_pointer(PointerPhase.Out, PointerType.Mouse, 0, 0, 0, 0);
});

export function worldPointer(
  phase: PointerPhase,
  pointerType: PointerType,
  id: i32,
  button: i32,
  x: f64,
  y: f64,
): void {
  const { dockMode, setDockMode } = useDockStore.getState();
  const typing = dockMode === DockMode.Message;
  switch (phase) {
    case PointerPhase.Down:
      pressed.set(id, pointerType);
      putKeyboardAway();
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
  kernel_pointer(phase, pointerType, id, button, x, y);
}

/** Each pointer still pressed ends with a cancel: the page lost it (blur,
 * hidden, the surface gone) and its lift will not come. */
export function releasePointers(): void {
  for (const [id, pointerType] of pressed) {
    worldPointer(PointerPhase.Cancel, pointerType, id, 0, 0, 0);
  }
}
