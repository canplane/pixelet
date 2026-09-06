// @pxl/web/app/store/ui_store.ts

import { create } from "zustand";

import type { f64 } from "@xpute/core/kit/word.ts";
import { cube_ids, type DecorId, decor_ids, type PlaceableCubeId } from "@pxl/shared/spec/cell/mod.ts";
import { DEFAULT_OBJECT_SIZE } from "@/modules/world/objects/size.ts";

export type ActiveTool = "pointer" | "object" | "decor" | "eraser";

// ts (Date.now() at append) lets ChatBubble derive its own remaining show
// time from wall-clock, not from when its own component happened to mount —
// without it, a character swap (Walker remounts under a new variant.key,
// see companion.tsx) resets ChatBubble's local state and it has no way to
// tell an old, already-dismissed line apart from a brand new one.
export type ChatLine = { id: number; text: string; ts: number };

const MAX_CHAT_LINES = 10;
let nextChatLineID = 0;

// mode_bar.tsx's hotbar (2026-09-07): used to be a fixed, always-complete
// enumeration (every flat color, plus its own separate decor row) — now that
// palette_modal.tsx's drawer holds the *entire* catalog (colors,
// terrain-styled cubes, decor), the hotbar's job shrunk to "whatever you
// actually reached for lately," across cube and decor alike, one shared MRU
// list instead of two side-by-side ones. Tagged (not a bare id union): a
// cube id and a decor id are both plain strings with no shared namespace, so
// telling them apart — and knowing whether picking one sets object_color_id
// or decor_id — needs the kind alongside the id, not just the id.
export type RecentPick = { kind: "object"; id: PlaceableCubeId } | { kind: "decor"; id: DecorId };

// 6, not the full 15-color/32-terrain-style/N-decor catalog: comfortably
// fits mobile's hotbar pill in one glance with zero horizontal scroll (see
// mode_bar.tsx's own width budget) — the whole point of moving to MRU
// instead of a static, ever-growing enumeration.
const MAX_RECENT_PICKS = 6;

function is_same_pick(a: RecentPick, b: RecentPick): boolean {
  return a.kind === b.kind && a.id === b.id;
}

function touch_recent(recent_picks: readonly RecentPick[], pick: RecentPick): RecentPick[] {
  return [pick, ...recent_picks.filter((p) => !is_same_pick(p, pick))].slice(0, MAX_RECENT_PICKS);
}

type UiState = {
  activeTool: ActiveTool;
  set_activeTool: (activeTool: ActiveTool) => void;

  // Widened to the full placeable catalog (2026-09-06, palette_modal.tsx's
  // own terrain-styled entries alongside mode_bar.tsx's original 15 flat
  // colors) — the hotbar only ever sets this to the narrower CubeColorId
  // subset, but the field itself has to hold whatever the modal picks too.
  object_color_id: PlaceableCubeId;

  object_size: f64;
  set_object_size: (object_size: f64) => void;

  decor_id: DecorId;

  // The hotbar's own shared MRU list (see RecentPick's own comment) — most
  // recent first. select_cube/select_decor are the only way to advance it;
  // there's no bare set_object_color_id/set_decor_id left; every real caller
  // (mode_bar's swatch/decor buttons, palette_modal's pick_cube/pick_decor)
  // wanted the activeTool switch and the recency bump every single time
  // anyway, so a setter that skipped either was never actually useful.
  recent_picks: RecentPick[];
  select_cube: (id: PlaceableCubeId) => void;
  select_decor: (id: DecorId) => void;

  chatLines: ChatLine[];
  append_chat_line: (text: string) => void;
};

export const useUiStore = create<UiState>((set) => ({
  activeTool: "pointer",
  set_activeTool: (activeTool) => set({ activeTool }),

  object_color_id: "red",

  object_size: DEFAULT_OBJECT_SIZE,
  set_object_size: (object_size) => set({ object_size }),

  decor_id: "tree",

  // Seeded with the first 5 flat colors plus the first decor kind — the
  // same lineup the hotbar used to show unconditionally before this went
  // in — so a fresh session's hotbar looks like the old static one at a
  // glance, and only starts reflecting real usage once something's picked.
  recent_picks: [
    ...cube_ids.slice(0, MAX_RECENT_PICKS - 1).map((id): RecentPick => ({ kind: "object", id })),
    { kind: "decor", id: decor_ids[0] },
  ],
  select_cube: (id) =>
    set((state) => ({
      object_color_id: id,
      activeTool: "object",
      recent_picks: touch_recent(state.recent_picks, { kind: "object", id }),
    })),
  select_decor: (id) =>
    set((state) => ({
      decor_id: id,
      activeTool: "decor",
      recent_picks: touch_recent(state.recent_picks, { kind: "decor", id }),
    })),

  chatLines: [],
  append_chat_line: (text) =>
    set((state) => ({
      chatLines: [...state.chatLines, { id: nextChatLineID++, text, ts: Date.now() }].slice(-MAX_CHAT_LINES),
    })),
}));
