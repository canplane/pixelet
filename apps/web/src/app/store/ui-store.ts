// @pxl/web/app/store/ui-store.ts

import { create } from "zustand";

import type { u32, u8 } from "@xpute/core/abi/word.ts";
import { cube_types, GroundChannelKey } from "@/catalog/cell/cube_type.spec.ts";
import { decor_kinds, placeable_cubes, type PlaceableCube, sameCube } from "@/catalog/cell/mod.ts";
import { ActiveTool, Command, encode_chat_create } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command } from "@/kernel/shell.ts";
import { kernel_hold } from "@/kernel/shell.ts";
import { storage_get, storage_set } from "@platform/storage.ts";

// ts (Date.now() at append) lets ChatBubble derive its own remaining show
// time from wall-clock, not from when its own component happened to mount —
// without it, a character swap (Walker remounts under a new variant.key)
// resets ChatBubble's local state and it has no way to
// tell an old, already-dismissed line apart from a brand new one.
interface ChatLine {
  id: number;
  text: string;
  ts: number;
}

const MAX_CHAT_LINES = 10;
let nextChatLineId = 0;

// mode-bar.tsx's hotbar is whatever was reached for lately, across cube and
// decor alike, one shared MRU list; the entire catalog (colors,
// terrain-styled cubes, decor) is drawer-modal.tsx's. Tagged by the tool that holds it: a
// cube and a decor kind are numbers of two catalogs, so which one a pick is —
// and whether picking it sets the cube or the decor kind — is the tool
// alongside it.
type RecentPick = { tool: ActiveTool.Object; cube: PlaceableCube } | { tool: ActiveTool.Decor; kind: u8 };

// 8, not the full 15-color/32-terrain-style/N-decor catalog: a recent list
// is short, and the drawer holds the rest. mode-bar.tsx's own strip already scrolls (overflow-x-auto) once the row
// overflows a narrow phone's width, so this doesn't need to fit in one
// glance with zero scroll the way a smaller cap might.
const MAX_RECENT_PICKS = 8;

function isSamePick(a: RecentPick, b: RecentPick): boolean {
  if (a.tool === ActiveTool.Object) return b.tool === ActiveTool.Object && sameCube(a.cube, b.cube);
  return b.tool === ActiveTool.Decor && a.kind === b.kind;
}

// A pick stored by an older build can name a cube or a decor kind the catalog
// no longer gives out — dropped here rather than trusted as-is, so a stale
// entry can't leave a dead one sitting in the hotbar forever.
function isKnownPick(pick: RecentPick): boolean {
  if (pick.tool === ActiveTool.Object) return placeable_cubes.some((cube) => sameCube(cube, pick.cube));
  return pick.tool === ActiveTool.Decor && decor_kinds.includes(pick.kind);
}

// Only ever inserts a genuinely new pick at the front — re-picking one
// already in the strip leaves the list untouched, so tapping an existing
// hotbar slot doesn't reshuffle the ones around it (confirmed live as
// disorienting: the strip's own order should only change when something new
// actually joins it, not every time you use it).
function touchRecent(recentPicks: readonly RecentPick[], pick: RecentPick): RecentPick[] {
  if (recentPicks.some((p) => isSamePick(p, pick))) return recentPicks as RecentPick[];
  return [pick, ...recentPicks].slice(0, MAX_RECENT_PICKS);
}

interface UiState {
  activeTool: ActiveTool;
  set_activeTool: (activeTool: ActiveTool) => void;

  /** The cube the object tool holds: any the drawer offers, not only the
   * hotbar's flat colors. */
  cube: PlaceableCube;

  /** The object size, by its tier in the kernel's `object_sizes`. */
  object_tier: u32;
  setObjectTier: (object_tier: u32) => void;

  /** The decor kind the decor tool holds, by its number. */
  decor: u8;

  // The hotbar's own shared MRU list (see RecentPick's own comment) — most
  // recent first. selectCube/selectDecor are the only way to advance it;
  // there's no bare setter for the cube or the decor kind; every real caller
  // (mode_bar's swatch/decor buttons, drawer_modal's pickCube/pickDecor)
  // wanted the activeTool switch and the recency bump every single time
  // anyway, so a setter that skipped either was never actually useful.
  recentPicks: RecentPick[];
  selectCube: (cube: PlaceableCube) => void;
  selectDecor: (kind: u8) => void;

  chatLines: ChatLine[];
  append_chat_line: (text: string) => void;
}

// One representative per placeable genre, not the first 5 flat colors — a
// fresh session's hotbar should hint at everything you *can* place (a plain
// color, and one of each terrain-styled channel) rather than just the color
// catalog's own first few entries, which only ever showed colors.
//
// Filled to MAX_RECENT_PICKS rather than left short: the strip's own width is
// the eight slots either way, so the empty tail read as missing rather than
// as room. The two that fill it are plain saturated colors, which is what the
// genre row above is least able to hint at — white alone does not say "there
// are colors in here".
const SEED: readonly RecentPick[] = [
  { tool: ActiveTool.Object, cube: { channel: null, type: cube_types.block.white } },
  { tool: ActiveTool.Object, cube: { channel: GroundChannelKey.LAND, type: cube_types.land.bare } },
  { tool: ActiveTool.Object, cube: { channel: GroundChannelKey.WATER, type: cube_types.water.river } },
  { tool: ActiveTool.Object, cube: { channel: GroundChannelKey.STRUCTURE, type: cube_types.structure.building } },
  { tool: ActiveTool.Object, cube: { channel: GroundChannelKey.TRANSPORT, type: cube_types.transport.road_default } },
  { tool: ActiveTool.Object, cube: { channel: null, type: cube_types.block.red } },
  { tool: ActiveTool.Object, cube: { channel: null, type: cube_types.block.blue } },
  { tool: ActiveTool.Decor, kind: decor_kinds[0] },
];

// Stored (platform/web/storage.ts) by the catalog's numbers: the cube, the decor kind
// and the picks — real picks worth remembering across a reload. The tool and
// the chat lines are left out: a load always starts at the neutral pointer
// tool, and the chat lines are a UI shell's, not the user's.
const STORAGE_KEY = "pxl:ui_hotbar";

interface Stored {
  cube: PlaceableCube;
  decor: u8;
  picks: RecentPick[];
}

function load(): Stored {
  let stored: Partial<Stored> | null = null;
  try {
    stored = JSON.parse(storage_get(STORAGE_KEY) ?? "null");
  } catch { /* nothing stored that reads */ }
  const picks = Array.isArray(stored?.picks) ? stored.picks.filter((p) => typeof p === "object" && p !== null && isKnownPick(p)) : [];
  // A short list is padded from the seed rather than left short. The strip is
  // eight slots wide whether or not they hold anything, so a missing tail
  // reads as broken rather than as room — and the padding is the first thing
  // to fall off the end once a real pick arrives, since touchRecent prepends
  // and slices.
  for (const seed of SEED) {
    if (picks.length >= MAX_RECENT_PICKS) break;
    if (!picks.some((pick) => isSamePick(pick, seed))) picks.push(seed);
  }
  const cube = placeable_cubes.find((c) => stored?.cube !== undefined && sameCube(c, stored.cube)) ?? { channel: null, type: cube_types.block.red };
  const decor = decor_kinds.find((kind) => kind === stored?.decor) ?? decor_kinds[0];
  return { cube, decor, picks };
}

function save(s: UiState): void {
  storage_set(STORAGE_KEY, JSON.stringify({ cube: s.cube, decor: s.decor, picks: s.recentPicks } satisfies Stored));
}

const stored = load();

export const useUiStore = create<UiState>()((set) => ({
  activeTool: ActiveTool.Pointer,
  set_activeTool: (activeTool) => {
    set({ activeTool });
  },

  cube: stored.cube,

  object_tier: 0,
  setObjectTier: (object_tier) => {
    set({ object_tier });
  },

  decor: stored.decor,

  recentPicks: stored.picks,
  selectCube: (cube) => {
    set((state) => ({
      cube,
      activeTool: ActiveTool.Object,
      recentPicks: touchRecent(state.recentPicks, { tool: ActiveTool.Object, cube }),
    }));
  },
  selectDecor: (kind) => {
    set((state) => ({
      decor: kind,
      activeTool: ActiveTool.Decor,
      recentPicks: touchRecent(state.recentPicks, { tool: ActiveTool.Decor, kind }),
    }));
  },

  chatLines: [],
  append_chat_line: (text) => {
    set((state) => ({
      chatLines: [...state.chatLines, { id: nextChatLineId++, text, ts: Date.now() }].slice(-MAX_CHAT_LINES),
    }));
    // The kernel draws the bubble over the companion from the last line.
    kernel_command(Command.CHAT_CREATE, encode_chat_create({ text }));
  },
}));

// What the pointer holds crosses to the kernel with each pointer event, by
// the catalog's numbers: the object tool holds the cube, the decor tool the
// decor kind.
function hold(s: UiState): void {
  if (s.activeTool === ActiveTool.Object) kernel_hold(s.activeTool, s.cube.channel, s.cube.type, s.object_tier);
  else kernel_hold(s.activeTool, null, s.activeTool === ActiveTool.Decor ? s.decor : 0, s.object_tier);
}

hold(useUiStore.getState());
useUiStore.subscribe((s, prev) => {
  if (s.activeTool !== prev.activeTool || s.cube !== prev.cube || s.decor !== prev.decor || s.object_tier !== prev.object_tier) hold(s);
  if (s.cube !== prev.cube || s.decor !== prev.decor || s.recentPicks !== prev.recentPicks) save(s);
});
