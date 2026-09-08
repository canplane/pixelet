// @pxl/web/app/store/ui_store.ts

import { create } from "zustand";
import { persist } from "zustand/middleware";

import type { f64 } from "@xpute/core/kit/word.ts";
import { decor_ids, type DecorId, placeable_cube_ids, type PlaceableCubeId } from "@pxl/shared/spec/cell/mod.ts";
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
// drawer_modal.tsx holds the *entire* catalog (colors,
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

// A pick persisted from an older session can name an id the catalog no
// longer has (a cube/decor kind renamed or removed since) — dropped here
// rather than trusted as-is, so a stale localStorage entry can't leave a
// dead id sitting in the hotbar forever.
function is_known_pick(pick: RecentPick): boolean {
  return pick.kind === "object" ? (placeable_cube_ids as readonly string[]).includes(pick.id) : (decor_ids as readonly string[]).includes(pick.id);
}

// Only ever inserts a genuinely new pick at the front — re-picking one
// already in the strip leaves the list untouched, so tapping an existing
// hotbar slot doesn't reshuffle the ones around it (confirmed live as
// disorienting: the strip's own order should only change when something new
// actually joins it, not every time you use it).
function touch_recent(recent_picks: readonly RecentPick[], pick: RecentPick): RecentPick[] {
  if (recent_picks.some((p) => is_same_pick(p, pick))) return recent_picks as RecentPick[];
  return [pick, ...recent_picks].slice(0, MAX_RECENT_PICKS);
}

type UiState = {
  activeTool: ActiveTool;
  set_activeTool: (activeTool: ActiveTool) => void;

  // Widened to the full placeable catalog (2026-09-06, drawer_modal.tsx's
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
  // (mode_bar's swatch/decor buttons, drawer_modal's pick_cube/pick_decor)
  // wanted the activeTool switch and the recency bump every single time
  // anyway, so a setter that skipped either was never actually useful.
  recent_picks: RecentPick[];
  select_cube: (id: PlaceableCubeId) => void;
  select_decor: (id: DecorId) => void;

  chatLines: ChatLine[];
  append_chat_line: (text: string) => void;
};

// Persisted (localStorage): object_color_id/decor_id/recent_picks — real
// picks worth remembering across a reload, mirroring settings_store.ts's
// own reasoning. activeTool and chatLines are deliberately left out
// (partialize below): a fresh load should always start at the neutral
// pointer tool, and chatLines is dev-only dummy data, not user state.
export const useUiStore = create<UiState>()(persist((set) => ({
  activeTool: "pointer",
  set_activeTool: (activeTool) => set({ activeTool }),

  object_color_id: "red",

  object_size: DEFAULT_OBJECT_SIZE,
  set_object_size: (object_size) => set({ object_size }),

  decor_id: "tree",

  // One representative per placeable genre, not the first 5 flat colors —
  // a fresh session's hotbar should hint at everything you *can* place
  // (a plain color, and one of each terrain-styled channel) rather than
  // just the color catalog's own first few entries, which only ever showed
  // colors. Each id is that channel's own first-declared key (cube.ts).
  recent_picks: [
    { kind: "object", id: "white" },
    { kind: "object", id: "land.BARE" },
    { kind: "object", id: "water.RIVER" },
    { kind: "object", id: "structure.BUILDING" },
    { kind: "object", id: "transport.ROAD_DEFAULT" },
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
}), {
  name: "pxl:ui_hotbar",
  partialize: (state) => ({
    object_color_id: state.object_color_id,
    decor_id: state.decor_id,
    recent_picks: state.recent_picks,
  }),
  // Default merge is a blind `{...current, ...persisted}` — fine for
  // settings_store.ts's own always-valid fields, not here: a persisted id
  // might no longer exist (is_known_pick above), and an empty persisted
  // recent_picks (every entry filtered out) should fall back to the fresh
  // default seeded above rather than leaving the hotbar empty.
  merge: (persisted, current) => {
    const p = persisted as Partial<{ object_color_id: PlaceableCubeId; decor_id: DecorId; recent_picks: RecentPick[] }> | null;
    if (!p) return current;
    const recent_picks = Array.isArray(p.recent_picks) ? p.recent_picks.filter(is_known_pick) : [];
    return {
      ...current,
      object_color_id: p.object_color_id && is_known_pick({ kind: "object", id: p.object_color_id }) ? p.object_color_id : current.object_color_id,
      decor_id: p.decor_id && is_known_pick({ kind: "decor", id: p.decor_id }) ? p.decor_id : current.decor_id,
      recent_picks: recent_picks.length > 0 ? recent_picks : current.recent_picks,
    };
  },
}));
