// @pxl/client/ui/feature/world/pointer.ts

/**
 * A touch that starts on the world puts the keyboard away. While the dock is
 * in message mode, that touch ends the typing instead of tapping: its lift
 * ends the gesture with no tap, and its drag still moves the world. In message
 * mode the pointer counts as off the world, so that nothing hovers there and
 * no tool previews what a tap would do.
 */

import type { f64, i32 } from "@xpute/kit/abi/word.ts";
import { PointerPhase, PointerType } from "@/abi/cmd.spec.ts";
import { putKeyboardAway } from "@/ui/shared/component/keyboard.tsx";
import {
  DockMode,
  dockMode,
  setDockMode,
  watchDockMode,
} from "@/ui/feature/world/dock/store.ts";
import * as input from "@/service/core/input.ts";

/** The pointers whose press ended the typing. Each one's lift is sent as a
 * cancel, so that it taps nothing. */
const spent = new Set<i32>();
/** The pressed pointers, each with its kind. Their moves drag rather than
 * hover. */
const pressed = new Map<i32, PointerType>();

watchDockMode((mode) => {
  input.typing(mode === DockMode.Message);
  if (mode === DockMode.Message)
    input.pointer(PointerPhase.Out, PointerType.Mouse, 0, 0, 0, 0);
});

export function worldPointer(
  phase: PointerPhase,
  pointerType: PointerType,
  id: i32,
  button: i32,
  x: f64,
  y: f64,
): void {
  const typing = dockMode() === DockMode.Message;
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
  input.pointer(phase, pointerType, id, button, x, y);
}

/** What the UI makes of a device's touch, which reaches the core without it.
 * A press puts the keyboard away and ends the typing; the core has already
 * spent that touch (INPUT_TYPING). */
export function worldTouched(phase: PointerPhase): void {
  if (phase !== PointerPhase.Down) return;
  putKeyboardAway();
  if (dockMode() === DockMode.Message) setDockMode(DockMode.Palette);
}

/** Ends every pressed pointer with a cancel. The caller calls it when the page
 * loses its pointers, on a blur, when it is hidden or when the surface goes,
 * because their lifts will not come. */
export function releasePointers(): void {
  for (const [id, pointerType] of pressed) {
    worldPointer(PointerPhase.Cancel, pointerType, id, 0, 0, 0);
  }
}
