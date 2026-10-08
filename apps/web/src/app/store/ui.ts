// @pxl/web/app/store/ui.ts

import { create } from "zustand";

import type { i32, u16, u32 } from "@xpute/core/abi/word.ts";
import { cube_types } from "@/catalog/cell/cube_type.spec.ts";
import { decorKinds, type PlaceableCube, placeableCubes } from "@/catalog/cell/mod.ts";
import { ActiveTool, Command, encode_chat_send, encode_input_hold_text } from "@/kernel/abi/cmd.spec.ts";
import { MARKER_TEXT_CHARS } from "@/catalog/cell/ext.spec.ts";
import { kernel_command, kernel_fits, kernel_hold, kernel_snapshot, useKernel } from "@/kernel/shell.ts";
import { storageGet, storageSet } from "@platform/storage.ts";
import { OBJECT_TIERS } from "@/config/object.spec.ts";

interface ChatLine {
  id: number;
  text: string;
}

const MAX_CHAT_LINES = 10;
let nextChatLineId = 0;

// The hotbar's MRU list, across cube and decor; the tool says which catalog a pick's number is in.
type RecentPick = { tool: ActiveTool.Object; cube: PlaceableCube } | { tool: ActiveTool.Decor; kind: u16 };

function isSamePick(a: RecentPick, b: RecentPick): boolean {
  if (a.tool === ActiveTool.Object) return b.tool === ActiveTool.Object && a.cube === b.cube;
  return b.tool === ActiveTool.Decor && a.kind === b.kind;
}

// A stored pick may name something the catalog no longer gives out.
function isKnownPick(pick: RecentPick): boolean {
  if (pick.tool === ActiveTool.Object) return placeableCubes.includes(pick.cube);
  return pick.tool === ActiveTool.Decor && decorKinds.includes(pick.kind);
}

// Only a new pick joins, at the front: slots reshuffling under the finger disorient.
function touchRecent(recentPicks: readonly RecentPick[], pick: RecentPick): RecentPick[] {
  if (recentPicks.some((p) => isSamePick(p, pick))) return recentPicks as RecentPick[];
  return [pick, ...recentPicks].slice(0, RECENT_PICKS);
}

/** What the pointer holds is the kernel's; the shell says what changes and reads it back. */
interface UiState {
  // Advanced only by selectCube/selectDecor, which also switch the tool.
  recentPicks: RecentPick[];
  selectCube: (cube: PlaceableCube) => void;
  selectDecor: (kind: u16) => void;

  chatLines: ChatLine[];
  appendChatLine: (text: string) => void;

  /** How far the dock's deck has turned, by a swipe or a key; the dock says which card that is. */
  dockTurn: i32;
  turnDock: (delta: i32) => void;

  /** A key's pick or size step, which the dock shows over the cube's swatch as a press would: its seed, or the sizes open. */
  swatchCue: SwatchCue;
  cueSwatch: (cube: PlaceableCube, open: boolean) => void;
  stepSize: (delta: i32) => void;
}

interface SwatchCue {
  /** Counts the cues, so the same one twice still shows. */
  n: u32;
  cube: PlaceableCube | null;
  open: boolean;
}

// One per placeable genre, so a fresh hotbar hints at everything placeable; as many as the hotbar holds.
const SEED: readonly RecentPick[] = [
  { tool: ActiveTool.Object, cube: cube_types.object.white },
  { tool: ActiveTool.Object, cube: cube_types.ground.land },
  { tool: ActiveTool.Object, cube: cube_types.ground.water },
  { tool: ActiveTool.Object, cube: cube_types.ground.building },
  { tool: ActiveTool.Object, cube: cube_types.ground.residential_road },
  { tool: ActiveTool.Object, cube: cube_types.object.red },
  { tool: ActiveTool.Object, cube: cube_types.object.blue },
  { tool: ActiveTool.Decor, kind: decorKinds[0] },
];

/** The hotbar's slots, always full: the seed fills it, and a new pick pushes the last out. */
export const RECENT_PICKS = SEED.length;

// Chat lines are not stored, nor what is held: a load starts at the pointer tool.
const STORAGE_KEY = "pxl:ui_hotbar";

interface Stored {
  picks: RecentPick[];
}

function load(): Stored {
  let stored: Partial<Stored> | null = null;
  try {
    stored = JSON.parse(storageGet(STORAGE_KEY) ?? "null");
  } catch { /* nothing stored that reads */ }
  const picks = Array.isArray(stored?.picks) ? stored.picks.filter((p) => typeof p === "object" && p !== null && isKnownPick(p)).slice(0, RECENT_PICKS) : [];
  // Padded from the seed: an empty tail reads as broken.
  for (const seed of SEED) {
    if (picks.length >= RECENT_PICKS) break;
    if (!picks.some((pick) => isSamePick(pick, seed))) picks.push(seed);
  }
  return { picks };
}

function save(s: UiState): void {
  storageSet(STORAGE_KEY, JSON.stringify({ picks: s.recentPicks } satisfies Stored));
}

/** What is held stays as the tool or the tier changes. */
function hold(tool: ActiveTool, tier: u32): void {
  const h = kernel_snapshot().holding;
  kernel_hold(tool, tool === ActiveTool.Decor ? h.decor : h.cube, tier);
}

export function holdTool(tool: ActiveTool): void {
  hold(tool, kernel_snapshot().holding.tier);
}

export function holdTier(tier: u32): void {
  hold(kernel_snapshot().holding.tool, tier);
}

const stored = load();

export const useUiStore = create<UiState>()((set) => ({
  recentPicks: stored.picks,
  selectCube: (cube) => {
    kernel_hold(ActiveTool.Object, cube, kernel_snapshot().holding.tier);
    set((state) => ({ recentPicks: touchRecent(state.recentPicks, { tool: ActiveTool.Object, cube }) }));
  },
  selectDecor: (kind) => {
    kernel_hold(ActiveTool.Decor, kind, kernel_snapshot().holding.tier);
    set((state) => ({ recentPicks: touchRecent(state.recentPicks, { tool: ActiveTool.Decor, kind }) }));
  },

  dockTurn: 0,
  turnDock: (delta) => set((state) => ({ dockTurn: state.dockTurn + delta })),

  swatchCue: { n: 0, cube: null, open: false },
  cueSwatch: (cube, open) => set((state) => ({ swatchCue: { n: state.swatchCue.n + 1, cube, open } })),
  stepSize: (delta) => {
    const h = kernel_snapshot().holding;
    if (h.tool !== ActiveTool.Object) return;
    holdTier(Math.min(OBJECT_TIERS - 1, Math.max(0, h.tier + delta)));
    useUiStore.getState().cueSwatch(h.cube, true);
  },

  chatLines: [],
  appendChatLine: (text) => {
    set((state) => ({
      chatLines: [...state.chatLines, { id: nextChatLineId++, text }].slice(-MAX_CHAT_LINES),
    }));
    kernel_command(Command.CHAT_SEND, encode_chat_send({ text }));
  },
}));

/** A line too long to send is refused whole, never cut. */
export function chatFits(text: string): boolean {
  return kernel_fits(encode_chat_send({ text }));
}

/** The words the marker tool puts up from now on, and words a tapped marker takes. */
export function holdMarkerText(text: string): void {
  kernel_command(Command.INPUT_HOLD_TEXT, encode_input_hold_text({ text }));
}

/** Words past the cap are refused whole, never cut. */
export function markerTextFits(text: string): boolean {
  return [...text].length <= MARKER_TEXT_CHARS;
}

export function useInHand(pick: RecentPick): boolean {
  return useKernel(({ holding: h }) => pick.tool === ActiveTool.Object ? h.tool === ActiveTool.Object && h.cube === pick.cube : h.tool === ActiveTool.Decor && h.decor === pick.kind);
}

useUiStore.subscribe((s, prev) => {
  if (s.recentPicks !== prev.recentPicks) save(s);
});
