// @pxl/client/shell/world/dock/store.ts

/** The dock's own state, which the kernel does not keep (what the pointer holds, the
 * hotbar's picks, the chat's lines, the dock's mode and its swatch's flash), and the
 * commands it sends with it: what is held, a chat line. */

import { create } from "zustand";

import { strerror } from "@xpute/kit/status/errno.spec.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import { ground, object } from "@/shell/catalog/asset/cube.spec.ts";
import { KIND } from "@/shell/catalog/asset/id.spec.ts";
import {
  allCubes,
  type AssetId,
  type Cube,
  decorIds,
  kindOf,
  MARKER,
} from "@/shell/catalog/asset/mod.ts";
import { Tool } from "@/abi/cmd.spec.ts";
import { OBJECT_SCALES } from "@/shell/world/object.spec.ts";
import { bracket } from "@/bracket.tsx";
import * as DockMode from "./mode.ts";
import * as edit from "@/port/edit.ts";
import * as scene from "@/port/scene.ts";

interface ChatLine {
  id: number;
  text: string;
}

const MAX_CHAT_LINES = 10;
let nextChatLineId = 0;

// A stored pick may name something the catalog no longer gives out.
function isKnownPick(id: AssetId): boolean {
  return id === MARKER || decorIds.includes(id) || allCubes.includes(id);
}

// Only a new pick joins, at the front.
function touchRecent(recentPicks: readonly AssetId[], id: AssetId): AssetId[] {
  if (recentPicks.includes(id)) return recentPicks as AssetId[];
  return [id, ...recentPicks].slice(0, RECENT_PICKS);
}

/** One of DockMode's. */
type DockMode = (typeof DockMode)[keyof typeof DockMode];
export { DockMode };

interface DockState {
  /** What the pointer holds, the shell's to choose: the kernel is told (`hold`). */
  tool: Tool;
  /** The asset last held. */
  id: AssetId;
  /** The scale last chosen for a cube, as a decor's `scale_steps`, kept as the tool changes: 0 a unit, each step down its half. */
  scaleSteps: i32;
  // The hotbar's MRU list; advanced only by selectAsset, which also switches the tool.
  recentPicks: AssetId[];
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
const SEED: readonly AssetId[] = [
  object.white,
  ground.land,
  ground.water,
  ground.building,
  ground.residential_road,
  object.red,
  object.blue,
  decorIds[0],
];

/** The hotbar's slots, always full: the seed fills it, and a new pick pushes the last out. */
export const RECENT_PICKS = SEED.length;

// Chat lines are not stored, nor what is held: a load starts at the pointer tool.
const STORAGE_KEY = "pxl:ui";

interface Stored {
  hotbar: AssetId[];
}

function load(): Stored {
  let stored: Partial<Stored> | null = null;
  try {
    stored = JSON.parse(bracket.storage.get(STORAGE_KEY) ?? "null");
  } catch {
    /* nothing stored that reads */
  }
  const picks = Array.isArray(stored?.hotbar)
    ? stored.hotbar
        .filter((p) => typeof p === "number" && isKnownPick(p))
        .slice(0, RECENT_PICKS)
    : [];
  // Padded from the seed.
  for (const seed of SEED) {
    if (picks.length >= RECENT_PICKS) break;
    if (!picks.includes(seed)) picks.push(seed);
  }
  return { hotbar: picks };
}

function save(s: DockState): void {
  bracket.storage.set(
    STORAGE_KEY,
    JSON.stringify({ hotbar: s.recentPicks } satisfies Stored),
  );
}

const store = create<DockState>()(() => ({
  tool: Tool.Pointer,
  id: 0,
  scaleSteps: 0,
  recentPicks: load().hotbar,
  chatLines: [],
  dockMode: DockMode.Palette,
  swatchFlash: { n: 0, cube: null, open: false },
}));

store.subscribe((s, prev) => {
  if (s.recentPicks !== prev.recentPicks) save(s);
});

export function useRecentPicks(): readonly AssetId[] {
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
export function recentPick(slot: number): AssetId {
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

/** Tells the kernel, and keeps it only where the kernel took it. */
function hold(tool: Tool, id: AssetId, scaleSteps: i32): boolean {
  const result = edit.tool(tool, id, scaleSteps);
  if (result < 0) {
    console.error(`[dock] the kernel refused the tool: ${strerror(result)}`);
    return false;
  }
  store.setState({ tool, id, scaleSteps });
  return true;
}

export function selectTool(tool: Tool): void {
  const { id, scaleSteps } = store.getState();
  hold(tool, id, scaleSteps);
}

export function selectScale(scaleSteps: i32): void {
  const { tool, id } = store.getState();
  hold(tool, id, scaleSteps);
}

/** Holds it to place, and puts it on the hotbar. */
export function selectAsset(id: AssetId): void {
  if (!hold(Tool.Item, id, store.getState().scaleSteps)) return;
  store.setState((s) => ({ recentPicks: touchRecent(s.recentPicks, id) }));
}

export function useTool(): Tool {
  return store((s) => s.tool);
}

export function useScale(): i32 {
  return store((s) => s.scaleSteps);
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

/** A step up, coarser, or down, finer, held to the object scales. */
export function stepScale(delta: i32): void {
  const { tool, id, scaleSteps } = store.getState();
  if (tool !== Tool.Item || kindOf(id) !== KIND.cube) return;
  selectScale(Math.min(0, Math.max(1 - OBJECT_SCALES, scaleSteps + delta)));
  flashSwatch(id, true);
}

export function appendChatLine(text: string): void {
  store.setState((s) => ({
    chatLines: [...s.chatLines, { id: nextChatLineId++, text }].slice(
      -MAX_CHAT_LINES,
    ),
  }));
  scene.chat(text);
}

export function useInHand(id: AssetId): boolean {
  return store((s) => s.tool === Tool.Item && s.id === id);
}
