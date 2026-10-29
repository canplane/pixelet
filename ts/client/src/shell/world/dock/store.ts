// @pxl/client/shell/world/dock/store.ts

/** The dock's own state, which the kernel does not keep (the hotbar's picks, the chat's lines, the
 * dock's mode and its swatch's flash), and the commands it sends with it: what is held, a chat line. */

import { create } from "zustand";

import type { i32, u16, u32 } from "@xpute/kit/abi/word.ts";
import { cube } from "@/shell/catalog/asset/cube.spec.ts";
import { allCubes, type Cube, decorIds } from "@/shell/catalog/asset/mod.ts";
import { ActiveTool, Command, encode_chat_send } from "@/abi/cmd.spec.ts";
import {
  kernel_command,
  kernel_hold,
  kernel_room,
  kernel_snapshot,
} from "@/abi/shell.ts";
import { OBJECT_TIERS } from "@/shell/world/object.spec.ts";
import { bracket } from "@/bracket.tsx";
import { useKernel } from "@/shell/store/watch.ts";

interface ChatLine {
  id: number;
  text: string;
}

const MAX_CHAT_LINES = 10;
let nextChatLineId = 0;

// The hotbar's MRU list, across cube and decor; the tool says which catalog a pick's number is in.
export type RecentPick =
  | { tool: ActiveTool.Object; cube: Cube }
  | {
      tool: ActiveTool.Decor;
      kind: u16;
    }
  | { tool: ActiveTool.Marker };

function isSamePick(a: RecentPick, b: RecentPick): boolean {
  if (a.tool === ActiveTool.Object) {
    return b.tool === ActiveTool.Object && a.cube === b.cube;
  }
  if (a.tool === ActiveTool.Decor) {
    return b.tool === ActiveTool.Decor && a.kind === b.kind;
  }
  return b.tool === ActiveTool.Marker;
}

// A stored pick may name something the catalog no longer gives out.
function isKnownPick(pick: RecentPick): boolean {
  if (pick.tool === ActiveTool.Object) return allCubes.includes(pick.cube);
  if (pick.tool === ActiveTool.Decor) return decorIds.includes(pick.kind);
  return pick.tool === ActiveTool.Marker;
}

// Only a new pick joins, at the front: slots reshuffling under the finger disorient.
function touchRecent(
  recentPicks: readonly RecentPick[],
  pick: RecentPick,
): RecentPick[] {
  if (recentPicks.some((p) => isSamePick(p, pick))) {
    return recentPicks as RecentPick[];
  }
  return [pick, ...recentPicks].slice(0, RECENT_PICKS);
}

/** What the dock is for. */
export const enum DockMode {
  Palette = 0,
  Message = 1,
}

/** What the pointer holds is the kernel's; the shell says what changes and reads it back. */
interface DockState {
  // Advanced only by selectCube/selectDecor/selectMarker, which also switch the tool.
  recentPicks: RecentPick[];
  chatLines: ChatLine[];
  /** What the dock is for: picking, at rest, or typing, which the keyboard follows. */
  dockMode: DockMode;
  /** A key's pick or size step, which the dock shows over the cube's swatch as a press would: its seed, or the sizes open. */
  swatchFlash: SwatchFlash;
}

interface SwatchFlash {
  /** Counts the flashes, so the same one twice still shows. */
  n: u32;
  cube: Cube | null;
  open: boolean;
}

// One per placeable genre, so a fresh hotbar hints at everything placeable; as many as the hotbar holds.
const SEED: readonly RecentPick[] = [
  { tool: ActiveTool.Object, cube: cube.object.white },
  { tool: ActiveTool.Object, cube: cube.ground.land },
  { tool: ActiveTool.Object, cube: cube.ground.water },
  { tool: ActiveTool.Object, cube: cube.ground.building },
  { tool: ActiveTool.Object, cube: cube.ground.residential_road },
  { tool: ActiveTool.Object, cube: cube.object.red },
  { tool: ActiveTool.Object, cube: cube.object.blue },
  { tool: ActiveTool.Decor, kind: decorIds[0] },
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
    stored = JSON.parse(bracket.storage.get(STORAGE_KEY) ?? "null");
  } catch {
    /* nothing stored that reads */
  }
  const picks = Array.isArray(stored?.picks)
    ? stored.picks
        .filter((p) => typeof p === "object" && p !== null && isKnownPick(p))
        .slice(0, RECENT_PICKS)
    : [];
  // Padded from the seed: an empty tail reads as broken.
  for (const seed of SEED) {
    if (picks.length >= RECENT_PICKS) break;
    if (!picks.some((pick) => isSamePick(pick, seed))) picks.push(seed);
  }
  return { picks };
}

function save(s: DockState): void {
  bracket.storage.set(
    STORAGE_KEY,
    JSON.stringify({ picks: s.recentPicks } satisfies Stored),
  );
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

const store = create<DockState>()(() => ({
  recentPicks: load().picks,
  chatLines: [],
  dockMode: DockMode.Palette,
  swatchFlash: { n: 0, cube: null, open: false },
}));

store.subscribe((s, prev) => {
  if (s.recentPicks !== prev.recentPicks) save(s);
});

export function useRecentPicks(): readonly RecentPick[] {
  return store((s) => s.recentPicks);
}

export function useChatLines(): readonly ChatLine[] {
  return store((s) => s.chatLines);
}

export function useDockMode(): DockMode {
  return store((s) => s.dockMode);
}

export function useSwatchFlash(): SwatchFlash {
  return store((s) => s.swatchFlash);
}

/** The hotbar's pick in `slot`, as it is now. */
export function recentPick(slot: number): RecentPick {
  return store.getState().recentPicks[slot];
}

/** What the dock is for now. */
export function dockMode(): DockMode {
  return store.getState().dockMode;
}

/** Called with the dock's mode and the one before it, on each change; the returned call stops it. */
export function watchDockMode(
  listener: (mode: DockMode, before: DockMode) => void,
): () => void {
  return store.subscribe((s, prev) => {
    if (s.dockMode !== prev.dockMode) listener(s.dockMode, prev.dockMode);
  });
}

function touch(pick: RecentPick): void {
  store.setState((s) => ({ recentPicks: touchRecent(s.recentPicks, pick) }));
}

export function selectCube(cube: Cube): void {
  kernel_hold(ActiveTool.Object, cube, kernel_snapshot().holding.tier);
  touch({ tool: ActiveTool.Object, cube });
}

export function selectDecor(kind: u16): void {
  kernel_hold(ActiveTool.Decor, kind, kernel_snapshot().holding.tier);
  touch({ tool: ActiveTool.Decor, kind });
}

export function selectMarker(): void {
  holdTool(ActiveTool.Marker);
  touch({ tool: ActiveTool.Marker });
}

export function setDockMode(mode: DockMode): void {
  store.setState({ dockMode: mode });
}

/** A swipe either way or Tab: the other mode. */
export function switchDockMode(): void {
  setDockMode(
    dockMode() === DockMode.Message ? DockMode.Palette : DockMode.Message,
  );
}

export function flashSwatch(cube: Cube, open: boolean): void {
  store.setState((s) => ({
    swatchFlash: { n: s.swatchFlash.n + 1, cube, open },
  }));
}

export function stepSize(delta: i32): void {
  const h = kernel_snapshot().holding;
  if (h.tool !== ActiveTool.Object) return;
  holdTier(Math.min(OBJECT_TIERS - 1, Math.max(0, h.tier + delta)));
  flashSwatch(h.cube, true);
}

export function appendChatLine(text: string): void {
  store.setState((s) => ({
    chatLines: [...s.chatLines, { id: nextChatLineId++, text }].slice(
      -MAX_CHAT_LINES,
    ),
  }));
  kernel_command(Command.CHAT_SEND, encode_chat_send(kernel_room, text));
}

export function useInHand(pick: RecentPick): boolean {
  return useKernel(({ holding: h }) => {
    if (pick.tool === ActiveTool.Object) {
      return h.tool === ActiveTool.Object && h.cube === pick.cube;
    }
    if (pick.tool === ActiveTool.Decor) {
      return h.tool === ActiveTool.Decor && h.decor === pick.kind;
    }
    return h.tool === ActiveTool.Marker;
  });
}
