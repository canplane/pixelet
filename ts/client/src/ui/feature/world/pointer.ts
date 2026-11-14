// @pxl/client/ui/feature/world/pointer.ts

/**
 * A touch that starts on the world puts the keyboard away. While the dock is
 * in message mode, that touch ends the typing instead of tapping: its lift
 * ends the gesture with no tap, and its drag still moves the world. In message
 * mode the pointer counts as off the world, so that nothing hovers there and
 * no tool previews what a tap would do.
 */

import type { f64, i32 } from "@xpute/kit/abi/word.ts";
import {
  type PointerPhase,
  type PointerType,
  POINTER_PHASE_CANCEL,
  POINTER_PHASE_DOWN,
  POINTER_PHASE_MOVE,
  POINTER_PHASE_OUT,
  POINTER_PHASE_UP,
  POINTER_TYPE_MOUSE,
} from "@/abi/input.enum.ts";
import { put_keyboard_away } from "@/ui/shared/component/keyboard.tsx";
import {
  DockMode,
  dock_mode,
  set_dock_mode,
  watch_dock_mode,
} from "@/ui/feature/world/dock/store.ts";
import * as input from "@/service/core/input.ts";

/** The pressed pointers, each with its kind. Their moves drag rather than
 * hover. */
const pressed = new Map<i32, PointerType>();

watch_dock_mode((mode) => {
  input.typing(mode === DockMode.MESSAGE);
  if (mode === DockMode.MESSAGE)
    input.pointer(POINTER_PHASE_OUT, POINTER_TYPE_MOUSE, 0, 0, 0, 0);
});

export function world_pointer(
  phase: PointerPhase,
  pointer_type: PointerType,
  id: i32,
  button: i32,
  x: f64,
  y: f64,
): void {
  const typing = dock_mode() === DockMode.MESSAGE;
  switch (phase) {
    case POINTER_PHASE_DOWN:
      // Sent while the core still types, so it spends the press (INPUT_TYPING).
      pressed.set(id, pointer_type);
      put_keyboard_away();
      input.pointer(phase, pointer_type, id, button, x, y);
      if (typing) set_dock_mode(DockMode.PALETTE);
      return;
    case POINTER_PHASE_MOVE:
      if (typing && !pressed.has(id)) return;
      break;
    case POINTER_PHASE_UP:
    case POINTER_PHASE_CANCEL:
      pressed.delete(id);
      break;
  }
  input.pointer(phase, pointer_type, id, button, x, y);
}

/** Ends every pressed pointer with a cancel. The caller calls it when the page
 * loses its pointers, on a blur, when it is hidden or when the surface goes,
 * because their lifts will not come. */
export function release_pointers(): void {
  for (const [id, pointer_type] of pressed) {
    world_pointer(POINTER_PHASE_CANCEL, pointer_type, id, 0, 0, 0);
  }
}
