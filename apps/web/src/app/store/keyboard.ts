// @pxl/web/app/store/keyboard.ts

/**
 * The keys the shell answers, each sent to whatever owns what it does: the
 * movement keys' direction to the kernel; a digit, a tool, the dock's turn and
 * the drawer to the shell's own stores. A key has no place on the screen to
 * route it as a pointer's has, so this is that routing: a sheet in front takes
 * every key but Escape, which closes it, and a field takes every key but Tab.
 * Keys are named by `code`, the key's place, so any layout works the same.
 */

import type { i32 } from "@xpute/core/abi/word.ts";
import { ActiveTool } from "@/kernel/abi/cmd.spec.ts";
import { kernel_keys } from "@/kernel/shell.ts";
import { closeModal, modalOpen, openModal } from "@/app/modal.tsx";
import { holdTool, useUiStore } from "@/app/store/ui.ts";

const MOVE: Record<string, [i32, i32]> = { KeyW: [0, -1], KeyS: [0, 1], KeyA: [-1, 0], KeyD: [1, 0] };

/** Digit1 to Digit8, the palette's slots in order. */
const SLOT = /^Digit([1-8])$/;

/** Figma's and Photoshop's letters for the move tool and the eraser. */
const TOOL: Record<string, ActiveTool> = { KeyV: ActiveTool.Pointer, KeyE: ActiveTool.Eraser };

/** A game's inventory key, and Figma's for its resources (Shift+I); Escape closes it as any sheet. */
const DRAWER = "KeyI";

const held = new Set<string>();
let sent: [i32, i32] = [0, 0];

function sendMove(): void {
  const to: [i32, i32] = [0, 0];
  for (const code of held) {
    to[0] += MOVE[code][0];
    to[1] += MOVE[code][1];
  }
  if (to[0] === sent[0] && to[1] === sent[1]) return;
  sent = to;
  kernel_keys(to[0], to[1]);
}

function pickSlot(slot: number): void {
  const { recentPicks, selectCube, selectDecor } = useUiStore.getState();
  const pick = recentPicks[slot];
  if (!pick) return;
  if (pick.tool === ActiveTool.Object) selectCube(pick.cube);
  else selectDecor(pick.kind);
}

/** Whether the key is one the shell answers, so a driver can leave the rest alone. */
export function keyDown(code: string, { shift, inField }: { shift: boolean; inField: boolean }): boolean {
  if (code === "Escape") {
    if (!modalOpen()) return false;
    closeModal();
    return true;
  }
  if (modalOpen()) return false;
  if (code === "Tab") {
    useUiStore.getState().turnDock(shift ? -1 : 1);
    return true;
  }
  if (inField) return false;
  if (code in TOOL) {
    holdTool(TOOL[code]);
    return true;
  }
  if (code === DRAWER) {
    openModal("drawer");
    return true;
  }
  if (code in MOVE) {
    held.add(code);
    sendMove();
    return true;
  }
  const slot = SLOT.exec(code);
  if (slot) {
    pickSlot(Number(slot[1]) - 1);
    return true;
  }
  return false;
}

export function keyUp(code: string): void {
  if (held.delete(code)) sendMove();
}

/** Focus went elsewhere: no key up will come for what is held. */
export function releaseKeys(): void {
  held.clear();
  sendMove();
}
