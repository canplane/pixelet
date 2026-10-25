// @pxl/client/shell/world/keyboard.ts

/**
 * The keys the shell answers, each sent to whatever owns what it does: the
 * movement keys' direction to the kernel; a digit, a tool, a size, the dock's
 * mode and the drawer to the shell's own stores. A key has no place on the screen to
 * route it as a pointer's has, so this is that routing: a sheet in front takes
 * every key but Escape, which closes it, and a field takes every key but Tab.
 * Keys are named by `code`, the key's place, so any layout works the same.
 */

import type { i32 } from "@xpute/kit/abi/word.ts";
import { ActiveTool } from "@/abi/cmd.spec.ts";
import { kernel_keys } from "@/abi/shell.ts";
import { closeModal, modalOpen, openModal } from "@/shell/modal.tsx";
import { holdTool, RECENT_PICKS, useUiStore } from "@/shell/store/ui.ts";

const MOVE: Record<string, [i32, i32]> = { KeyW: [0, -1], KeyS: [0, 1], KeyA: [-1, 0], KeyD: [1, 0] };

/** Digit1 on, the hotbar's slots in order. */
const SLOT = /^Digit([1-9])$/;

/** Figma's and Photoshop's letters for the move tool and the eraser. */
const TOOL: Record<string, ActiveTool> = { KeyV: ActiveTool.Pointer, KeyE: ActiveTool.Eraser };

/** A game's inventory key, and Figma's for its resources (Shift+I); Escape closes it as any sheet. */
const DRAWER = "KeyI";

/** Photoshop's brush size: `]` a coarser block, `[` a finer, a tier being finer as it counts up. */
const SIZE: Record<string, i32> = { BracketRight: -1, BracketLeft: 1 };

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
  const { recentPicks, selectCube, selectDecor, selectMarker, cueSwatch } = useUiStore.getState();
  const pick = recentPicks[slot];
  if (pick.tool === ActiveTool.Decor) return selectDecor(pick.kind);
  if (pick.tool === ActiveTool.Marker) return selectMarker();
  selectCube(pick.cube);
  cueSwatch(pick.cube, false);
}

/** Whether the key is one the shell answers, so a driver can leave the rest alone. */
export function keyDown(code: string, { inField }: { inField: boolean }): boolean {
  if (code === "Escape") {
    if (!modalOpen()) return false;
    closeModal();
    return true;
  }
  if (modalOpen()) return false;
  if (code === "Tab") {
    useUiStore.getState().switchDockMode();
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
  if (code in SIZE) {
    useUiStore.getState().stepSize(SIZE[code]);
    return true;
  }
  if (code in MOVE) {
    held.add(code);
    sendMove();
    return true;
  }
  const slot = Number(SLOT.exec(code)?.[1] ?? 0);
  if (slot >= 1 && slot <= RECENT_PICKS) {
    pickSlot(slot - 1);
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
