// @pxl/web/app/store/ui.ts

import { create } from "zustand";

import type { i32, u32, u8 } from "@xpute/core/abi/word.ts";
import type { HoldingState } from "@/kernel/abi/cmd.spec.ts";
import { cube_types, GroundChannelKey } from "@/catalog/cell/cube_type.spec.ts";
import { decorKinds, type PlaceableCube, placeableCubes, sameCube } from "@/catalog/cell/mod.ts";
import { ActiveTool, Command, encode_chat_send } from "@/kernel/abi/cmd.spec.ts";
import { storage, STORAGE_KEYS } from "@/kernel/abi/host.spec.ts";
import { kernel_command, kernel_hold, kernel_snapshot, useKernel } from "@/kernel/shell.ts";
import { storageGet, storageSet } from "@platform/storage.ts";

interface ChatLine {
  id: number;
  text: string;
}

const MAX_CHAT_LINES = 10;
let nextChatLineId = 0;

// The hotbar's MRU list, across cube and decor; the tool says which catalog a pick's number is in.
type RecentPick = { tool: ActiveTool.Object; cube: PlaceableCube } | { tool: ActiveTool.Decor; kind: u8 };

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

/** What the pointer holds is the kernel's; the shell says what changes and reads it back. */
interface UiState {
  // Advanced only by selectCube/selectDecor, which also switch the tool.
  recentPicks: RecentPick[];
  selectCube: (cube: PlaceableCube) => void;
  selectDecor: (kind: u8) => void;

  chatLines: ChatLine[];
  appendChatLine: (text: string) => void;

  /** How far the dock's deck has turned, by a swipe or a key; the dock says which card that is. */
  dockTurn: i32;
  turnDock: (delta: i32) => void;
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

// Chat lines are not stored, nor what is held: a load starts at the pointer tool.
const STORAGE_KEY = STORAGE_KEYS[storage.HOTBAR];

interface Stored {
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
  return { picks };
}

function save(s: UiState): void {
  storageSet(STORAGE_KEY, JSON.stringify({ picks: s.recentPicks } satisfies Stored));
}

/** The tier stays as the tool changes. */
export function holdTool(tool: ActiveTool): void {
  kernel_hold(tool, null, 0, kernel_snapshot().holding.tier);
}

export function holdTier(tier: u32): void {
  const h = kernel_snapshot().holding;
  kernel_hold(h.tool, h.channel, h.tool === ActiveTool.Decor ? h.decor : h.cube, tier);
}

export function heldCube(h: HoldingState): PlaceableCube {
  return { channel: h.channel, type: h.cube };
}

const stored = load();

export const useUiStore = create<UiState>()((set) => ({
  recentPicks: stored.picks,
  selectCube: (cube) => {
    kernel_hold(ActiveTool.Object, cube.channel, cube.type, kernel_snapshot().holding.tier);
    set((state) => ({ recentPicks: touchRecent(state.recentPicks, { tool: ActiveTool.Object, cube }) }));
  },
  selectDecor: (kind) => {
    kernel_hold(ActiveTool.Decor, null, kind, kernel_snapshot().holding.tier);
    set((state) => ({ recentPicks: touchRecent(state.recentPicks, { tool: ActiveTool.Decor, kind }) }));
  },

  dockTurn: 0,
  turnDock: (delta) => set((state) => ({ dockTurn: state.dockTurn + delta })),

  chatLines: [],
  appendChatLine: (text) => {
    set((state) => ({
      chatLines: [...state.chatLines, { id: nextChatLineId++, text }].slice(-MAX_CHAT_LINES),
    }));
    kernel_command(Command.CHAT_SEND, encode_chat_send({ text }));
  },
}));

export function useInHand(pick: RecentPick): boolean {
  return useKernel(({ holding: h }) => pick.tool === ActiveTool.Object ? h.tool === ActiveTool.Object && sameCube(heldCube(h), pick.cube) : h.tool === ActiveTool.Decor && h.decor === pick.kind);
}

useUiStore.subscribe((s, prev) => {
  if (s.recentPicks !== prev.recentPicks) save(s);
});
