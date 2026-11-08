// @pxl/client/ui/world/dock/store.ts

/**
 * The dock's state, which the UI owns: the tool and the asset in hand, the
 * hotbar, the chat lines, the dock's mode and the swatch flash. The store
 * tells the core what is held and sends it each chat line.
 */

import { create } from "zustand";

import { strerror } from "@xpute/kit/status/errno.spec.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import { ground, object } from "@/ui/catalog/asset/cube.spec.ts";
import { KIND } from "@/ui/catalog/asset/id.spec.ts";
import {
  allCubes,
  type AssetId,
  type Cube,
  decorIds,
  kindOf,
  MARKER,
} from "@/ui/catalog/asset/mod.ts";
import { Tool } from "@/abi/cmd.spec.ts";
import { OBJECT_SCALES } from "@/ui/world/object.spec.ts";
import * as storage from "@/service/storage.ts";
import * as DockMode from "./mode.ts";
import * as edit from "@/port/edit.ts";
import * as scene from "@/port/scene.ts";

interface ChatLine {
  id: number;
  text: string;
}

const MAX_CHAT_LINES = 10;
let nextChatLineId = 0;

// A saved hotbar may come from another build, with ids this catalog lacks.
function isKnownPick(id: AssetId): boolean {
  return id === MARKER || decorIds.includes(id) || allCubes.includes(id);
}

// A pick already on the hotbar keeps its slot. A new pick goes in front and
// pushes the last one out.
function touchRecent(recentPicks: readonly AssetId[], id: AssetId): AssetId[] {
  if (recentPicks.includes(id)) return recentPicks as AssetId[];
  return [id, ...recentPicks].slice(0, RECENT_PICKS);
}

type DockMode = (typeof DockMode)[keyof typeof DockMode];
export { DockMode };

interface DockState {
  /** The tool in hand. The UI chooses it and tells the core through `hold`. */
  tool: Tool;
  /** The asset last held, kept while another tool is in hand. */
  id: AssetId;
  /**
   * The scale last chosen for a cube, kept while the tool changes. It counts
   * as a decor's `scale_steps` does: 0 is a unit, and each step down halves it.
   */
  scaleSteps: i32;
  // The hotbar, newest pick first. Only selectAsset adds a pick, and only once
  // the core holds it.
  recentPicks: AssetId[];
  chatLines: ChatLine[];
  /** Whether the dock shows the palette or the message field. The keyboard is
   * up while it shows the message field (message.tsx). */
  dockMode: DockMode;
  /** A pick or a size step made with a key. The dock shows it over the cube's
   * swatch as a press would (popover.tsx). */
  swatchFlash: SwatchFlash;
}

interface SwatchFlash {
  /** Counts the flashes, so that the same flash twice in a row shows twice. */
  n: u32;
  cube: Cube | null;
  open: boolean;
}

// One asset of each kind that can be placed, so that a fresh hotbar shows the
// whole range. Its length sets the hotbar's size.
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

/** The number of hotbar slots. The hotbar is always full: the seed fills the
 * empty slots, and a new pick pushes the last one out. */
export const RECENT_PICKS = SEED.length;

// Only the hotbar is saved. Chat lines and the tool in hand are not, so every
// load starts with the pointer tool.
const STORAGE_KEY = "pxl:ui";

interface Stored {
  hotbar: AssetId[];
}

function load(): Stored {
  let stored: Partial<Stored> | null = null;
  try {
    stored = JSON.parse(storage.get(STORAGE_KEY) ?? "null");
  } catch {
    /* A missing or unreadable value leaves the hotbar to the seed. */
  }
  const picks = Array.isArray(stored?.hotbar)
    ? stored.hotbar
        .filter((p) => typeof p === "number" && isKnownPick(p))
        .slice(0, RECENT_PICKS)
    : [];
  for (const seed of SEED) {
    if (picks.length >= RECENT_PICKS) break;
    if (!picks.includes(seed)) picks.push(seed);
  }
  return { hotbar: picks };
}

function save(s: DockState): void {
  storage.set(
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

/** The pick in hotbar `slot`, read once rather than watched. */
export function recentPick(slot: number): AssetId {
  return store.getState().recentPicks[slot];
}

/** The dock's mode, read once rather than watched. */
export function dockMode(): DockMode {
  return store.getState().dockMode;
}

export function watchDockMode(
  listener: (mode: DockMode, before: DockMode) => void,
): () => void {
  return store.subscribe((s, prev) => {
    if (s.dockMode !== prev.dockMode) listener(s.dockMode, prev.dockMode);
  });
}

/** Tells the core what is held, and keeps it only if the core accepts it, so
 * that the dock never shows a tool the core refused. */
async function hold(
  tool: Tool,
  id: AssetId,
  scaleSteps: i32,
): Promise<boolean> {
  const result = await edit.tool(tool, id, scaleSteps);
  if (result < 0) {
    console.error(`[dock] the core refused the tool: ${strerror(result)}`);
    return false;
  }
  store.setState({ tool, id, scaleSteps });
  return true;
}

export function selectTool(tool: Tool): void {
  const { id, scaleSteps } = store.getState();
  void hold(tool, id, scaleSteps);
}

export function selectScale(scaleSteps: i32): void {
  const { tool, id } = store.getState();
  void hold(tool, id, scaleSteps);
}

/** Puts `id` in hand to place, and on the hotbar once the core holds it. */
export function selectAsset(id: AssetId): void {
  void hold(Tool.Item, id, store.getState().scaleSteps).then((held) => {
    if (held) {
      store.setState((s) => ({ recentPicks: touchRecent(s.recentPicks, id) }));
    }
  });
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

/** Turns to the other mode. A swipe either way and the Tab key both do. */
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

/** Steps the scale of the cube in hand. A `delta` of 1 is one step coarser,
 * and -1 one step finer. */
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
