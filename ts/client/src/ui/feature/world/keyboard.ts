// @pxl/client/ui/feature/world/keyboard.ts

/**
 * A key has no place on the screen that decides what receives it, as a
 * pointer has, so this file decides. A sheet in front takes every key except
 * Escape, which closes it. A text field takes every key except Tab and
 * Escape, and both of those leave the message for the palette. Keys are
 * named by `code`, the key's physical place, so that every keyboard layout
 * works the same.
 */

import type { i32 } from "@xpute/kit/abi/word.ts";
import { type Tool, TOOL_ERASER, TOOL_POINTER } from "@/abi/edit.enum.ts";
import { KIND_CUBE } from "@/ui/shared/asset/id.spec.ts";
import { kind_of } from "@/ui/shared/asset/catalog.ts";
import { close_modal, modal_open, open_modal } from "@/ui/shared/modal.ts";
import {
  DockMode,
  dock_mode,
  flash_swatch,
  select_tool,
  RECENT_PICKS,
  recent_pick,
  select_asset,
  set_dock_mode,
  step_scale,
  switch_dock_mode,
} from "@/ui/feature/world/dock/store.ts";
import * as input from "@/service/core/input.ts";

const MOVE: Record<string, [i32, i32]> = {
  KeyW: [0, -1],
  KeyS: [0, 1],
  KeyA: [-1, 0],
  KeyD: [1, 0],
};

const SLOT = /^Digit([1-9])$/;

/** The letters follow Figma and Photoshop: V for the move tool and E for the
 * eraser. */
const TOOL: Record<string, Tool> = {
  KeyV: TOOL_POINTER,
  KeyE: TOOL_ERASER,
};

/** I opens the drawer, as it opens a game's inventory and, with Shift,
 * Figma's resources. */
const DRAWER = "KeyI";

/** The brackets size the block as they size Photoshop's brush: `]` makes it
 * coarser and `[` finer. */
const SIZE: Record<string, i32> = { BracketRight: 1, BracketLeft: -1 };

const held = new Set<string>();
let sent: [i32, i32] = [0, 0];

function send_move(): void {
  const to: [i32, i32] = [0, 0];
  for (const code of held) {
    to[0] += MOVE[code][0];
    to[1] += MOVE[code][1];
  }
  if (to[0] === sent[0] && to[1] === sent[1]) return;
  sent = to;
  input.move(to[0], to[1]);
}

function pick_slot(slot: number): void {
  const id = recent_pick(slot);
  select_asset(id);
  if (kind_of(id) === KIND_CUBE) flash_swatch(id, false);
}

/** Returns whether the UI answers the key, so that the caller can leave every
 * other key to its default. */
export function key_down(
  code: string,
  { in_field }: { in_field: boolean },
): boolean {
  if (code === "Escape") {
    if (modal_open()) {
      close_modal();
      return true;
    }
    if (dock_mode() !== DockMode.Message) return false;
    set_dock_mode(DockMode.Palette);
    return true;
  }
  if (modal_open()) return false;
  if (code === "Tab") {
    switch_dock_mode();
    return true;
  }
  if (in_field) return false;
  if (code in TOOL) {
    select_tool(TOOL[code]);
    return true;
  }
  if (code === DRAWER) {
    open_modal("drawer");
    return true;
  }
  if (code in SIZE) {
    step_scale(SIZE[code]);
    return true;
  }
  if (code in MOVE) {
    held.add(code);
    send_move();
    return true;
  }
  const slot = Number(SLOT.exec(code)?.[1] ?? 0);
  if (slot >= 1 && slot <= RECENT_PICKS) {
    pick_slot(slot - 1);
    return true;
  }
  return false;
}

export function key_up(code: string): void {
  if (held.delete(code)) send_move();
}

/** Releases every held key. The caller calls it when focus leaves, because no
 * key up will come for the keys held then. */
export function release_keys(): void {
  held.clear();
  send_move();
}
