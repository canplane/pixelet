// @pxl/web/app/store/ui-store.ts

import { create } from "zustand";

import type { u32, u8 } from "@xpute/core/abi/word.ts";
import { cube_types, GroundChannelKey } from "@/catalog/cell/cube_type.spec.ts";
import { decorKinds, type PlaceableCube, placeableCubes, sameCube } from "@/catalog/cell/mod.ts";
import { ActiveTool, Command, encode_chat_send } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command } from "@/kernel/shell.ts";
import { kernel_hold } from "@/kernel/shell.ts";
import { storageGet, storageSet } from "@platform/storage.ts";

interface ChatLine {
  id: number;
  text: string;
}

const MAX_CHAT_LINES = 10;
let nextChatLineId = 0;

// The hotbar's MRU list, across cube and decor; the tool says which catalog a pick's number is in.
export type RecentPick = { tool: ActiveTool.Object; cube: PlaceableCube } | { tool: ActiveTool.Decor; kind: u8 };

const MAX_RECENT_PICKS = 8;

function isSamePick(a: RecentPick, b: RecentPick): boolean {
  if (a.tool === ActiveTool.Object) return b.tool === ActiveTool.Object && sameCube(a.cube, b.cube);
  return b.tool === ActiveTool.Decor && a.kind === b.kind;
}

// A stored pick may name something the catalog no longer gives out.
function isKnownPick(pick: RecentPick): boolean {
  if (pick.tool === ActiveTool.Object) return placeableCubes.some((cube) => sameCube(cube, pick.cube));
  return pick.tool === ActiveTool.Decor && decorKinds.includes(pick.kind);
}

// Only a new pick joins, at the front: slots reshuffling under the finger disorient.
function touchRecent(recentPicks: readonly RecentPick[], pick: RecentPick): RecentPick[] {
  if (recentPicks.some((p) => isSamePick(p, pick))) return recentPicks as RecentPick[];
  return [pick, ...recentPicks].slice(0, MAX_RECENT_PICKS);
}

interface UiState {
  activeTool: ActiveTool;
  set_activeTool: (activeTool: ActiveTool) => void;

  cube: PlaceableCube;

  objectTier: u32;
  setObjectTier: (objectTier: u32) => void;

  decor: u8;

  // Advanced only by selectCube/selectDecor, which also switch the tool.
  recentPicks: RecentPick[];
  selectCube: (cube: PlaceableCube) => void;
  selectDecor: (kind: u8) => void;

  chatLines: ChatLine[];
  appendChatLine: (text: string) => void;
}

// One per placeable genre, so a fresh hotbar hints at everything placeable, padded to MAX_RECENT_PICKS.
const SEED: readonly RecentPick[] = [
  { tool: ActiveTool.Object, cube: { channel: null, type: cube_types.block.white } },
  { tool: ActiveTool.Object, cube: { channel: GroundChannelKey.LAND, type: cube_types.land.bare } },
  { tool: ActiveTool.Object, cube: { channel: GroundChannelKey.WATER, type: cube_types.water.river } },
  { tool: ActiveTool.Object, cube: { channel: GroundChannelKey.STRUCTURE, type: cube_types.structure.building } },
  { tool: ActiveTool.Object, cube: { channel: GroundChannelKey.TRANSPORT, type: cube_types.transport.road_default } },
  { tool: ActiveTool.Object, cube: { channel: null, type: cube_types.block.red } },
  { tool: ActiveTool.Object, cube: { channel: null, type: cube_types.block.blue } },
  { tool: ActiveTool.Decor, kind: decorKinds[0] },
];

// The tool and chat lines are not stored: a load starts at the pointer tool.
const STORAGE_KEY = "pxl:ui_hotbar";

interface Stored {
  cube: PlaceableCube;
  decor: u8;
  picks: RecentPick[];
}

function load(): Stored {
  let stored: Partial<Stored> | null = null;
  try {
    stored = JSON.parse(storageGet(STORAGE_KEY) ?? "null");
  } catch { /* nothing stored that reads */ }
  const picks = Array.isArray(stored?.picks) ? stored.picks.filter((p) => typeof p === "object" && p !== null && isKnownPick(p)) : [];
  // Padded from the seed: an empty tail reads as broken.
  for (const seed of SEED) {
    if (picks.length >= MAX_RECENT_PICKS) break;
    if (!picks.some((pick) => isSamePick(pick, seed))) picks.push(seed);
  }
  const cube = placeableCubes.find((c) => stored?.cube !== undefined && sameCube(c, stored.cube)) ?? { channel: null, type: cube_types.block.red };
  const decor = decorKinds.find((kind) => kind === stored?.decor) ?? decorKinds[0];
  return { cube, decor, picks };
}

function save(s: UiState): void {
  storageSet(STORAGE_KEY, JSON.stringify({ cube: s.cube, decor: s.decor, picks: s.recentPicks } satisfies Stored));
}

const stored = load();

export const useUiStore = create<UiState>()((set) => ({
  activeTool: ActiveTool.Pointer,
  set_activeTool: (activeTool) => {
    set({ activeTool });
  },

  cube: stored.cube,

  objectTier: 0,
  setObjectTier: (objectTier) => {
    set({ objectTier });
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
  appendChatLine: (text) => {
    set((state) => ({
      chatLines: [...state.chatLines, { id: nextChatLineId++, text }].slice(-MAX_CHAT_LINES),
    }));
    kernel_command(Command.CHAT_SEND, encode_chat_send({ text }));
  },
}));

export function useInHand(pick: RecentPick): boolean {
  return useUiStore((s) => pick.tool === ActiveTool.Object ? s.activeTool === ActiveTool.Object && sameCube(s.cube, pick.cube) : s.activeTool === ActiveTool.Decor && s.decor === pick.kind);
}

function hold(s: UiState): void {
  if (s.activeTool === ActiveTool.Object) kernel_hold(s.activeTool, s.cube.channel, s.cube.type, s.objectTier);
  else kernel_hold(s.activeTool, null, s.activeTool === ActiveTool.Decor ? s.decor : 0, s.objectTier);
}

hold(useUiStore.getState());
useUiStore.subscribe((s, prev) => {
  if (s.activeTool !== prev.activeTool || s.cube !== prev.cube || s.decor !== prev.decor || s.objectTier !== prev.objectTier) hold(s);
  if (s.cube !== prev.cube || s.decor !== prev.decor || s.recentPicks !== prev.recentPicks) save(s);
});
