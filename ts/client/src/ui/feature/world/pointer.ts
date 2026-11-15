// @pxl/client/ui/feature/world/pointer.ts

/**
 * A touch that starts on the world puts the keyboard away. While the dock is
 * in message mode, that touch ends the typing instead of tapping: its lift
 * goes to the core as a cancel, which ends the gesture with no tap, and its
 * drag still moves the world. In message mode the pointer counts as off the
 * world, so that nothing hovers there and no tool previews what a tap would
 * do.
 */

import type { f64, i32 } from "@xpute/kit/abi/word.ts";
import {
  type Drag,
  type PointerPhase,
  DRAG_NONE,
  POINTER_PHASE_CANCEL,
  POINTER_PHASE_DOWN,
  POINTER_PHASE_MOVE,
  POINTER_PHASE_OUT,
  POINTER_PHASE_UP,
} from "@/abi/input.enum.ts";
import { put_keyboard_away } from "@/ui/shared/component/keyboard.tsx";
import {
  DockMode,
  dock_mode,
  set_dock_mode,
  watch_dock_mode,
} from "@/ui/feature/world/dock/store.ts";
import * as input from "@/service/core/input.ts";

/** The pressed pointers, each with whether it is a finger. Their moves drag
 * rather than hover. */
const pressed = new Map<i32, boolean>();

/** The pressed pointers that ended the typing, whose lifts tap nothing. */
const spent = new Set<i32>();

watch_dock_mode((mode) => {
  if (mode === DockMode.MESSAGE)
    input.pointer(POINTER_PHASE_OUT, false, 0, DRAG_NONE, 0, 0);
});

export function world_pointer(
  phase: PointerPhase,
  touch: boolean,
  id: i32,
  drag: Drag,
  x: f64,
  y: f64,
): void {
  const typing = dock_mode() === DockMode.MESSAGE;
  switch (phase) {
    case POINTER_PHASE_DOWN:
      pressed.set(id, touch);
      if (typing) spent.add(id);
      put_keyboard_away();
      input.pointer(phase, touch, id, drag, x, y);
      if (typing) set_dock_mode(DockMode.PALETTE);
      return;
    case POINTER_PHASE_MOVE:
      if (typing && !pressed.has(id)) return;
      break;
    case POINTER_PHASE_UP:
    case POINTER_PHASE_CANCEL:
      pressed.delete(id);
      if (spent.delete(id)) {
        input.pointer(POINTER_PHASE_CANCEL, touch, id, drag, x, y);
        return;
      }
      break;
  }
  input.pointer(phase, touch, id, drag, x, y);
}

/** Ends every pressed pointer with a cancel. The caller calls it when the page
 * loses its pointers, on a blur, when it is hidden or when the surface goes,
 * because their lifts will not come. */
export function release_pointers(): void {
  for (const [id, touch] of pressed) {
    world_pointer(POINTER_PHASE_CANCEL, touch, id, DRAG_NONE, 0, 0);
  }
}
