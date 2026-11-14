// @pxl/client/ui/feature/world/dock/store.ts

import { create } from "zustand";

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import {
  GROUND_BUILDING,
  GROUND_LAND,
  GROUND_RESIDENTIAL_ROAD,
  GROUND_WATER,
  OBJECT_BLUE,
  OBJECT_RED,
  OBJECT_WHITE,
} from "@/abi/cube.spec.ts";
import { DECORS } from "@/ui/shared/asset/decor.spec.ts";
import { KIND_CUBE } from "@/ui/shared/asset/id.spec.ts";
import {
  type AssetId,
  type Cube,
  CUBES,
  kindOf,
  MARKER,
} from "@/ui/shared/asset/catalog.ts";
import { type Tool, TOOL_ITEM, TOOL_POINTER } from "@/abi/edit.enum.ts";
import { OBJECT_SCALE_CNT } from "@/ui/feature/world/object.spec.ts";
import * as storage from "@/service/platform/storage.ts";
import * as DockMode from "./mode.ts";
import * as edit from "@/service/core/edit.ts";
import * as scene from "@/service/core/scene.ts";

interface ChatLine {
  id: number;
  text: string;
}

const MAX_CHAT_LINES = 10;
let nextChatLineId = 0;

// A saved hotbar may come from another build, with ids this catalog lacks.
function isKnownPick(id: AssetId): boolean {
  return id === MARKER || DECORS.includes(id) || CUBES.includes(id);
}

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
   * as a decor's `scale_step` does: 0 is a unit, and each step down halves it.
   */
  scaleStep: i32;
  // The hotbar, newest pick first. Only selectAsset adds a pick, and only once
  // the core holds it.
  recentPicks: AssetId[];
  chatLines: ChatLine[];
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
  OBJECT_WHITE,
  GROUND_LAND,
  GROUND_WATER,
  GROUND_BUILDING,
  GROUND_RESIDENTIAL_ROAD,
  OBJECT_RED,
  OBJECT_BLUE,
  DECORS[0],
];

/** The hotbar is always full: the seed fills the empty slots, and a new pick
 * pushes the last one out. */
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
  tool: TOOL_POINTER,
  id: 0,
  scaleStep: 0,
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

export function recentPick(slot: number): AssetId {
  return store.getState().recentPicks[slot];
}

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
async function hold(tool: Tool, id: AssetId, scaleStep: i32): Promise<boolean> {
  try {
    await edit.tool(tool, id, scaleStep);
  } catch (err) {
    console.error(`[dock] the core refused the tool: ${String(err)}`);
    return false;
  }
  store.setState({ tool, id, scaleStep });
  return true;
}

export function selectTool(tool: Tool): void {
  const { id, scaleStep } = store.getState();
  void hold(tool, id, scaleStep);
}

export function selectScale(scaleStep: i32): void {
  const { tool, id } = store.getState();
  void hold(tool, id, scaleStep);
}

export function selectAsset(id: AssetId): void {
  void hold(TOOL_ITEM, id, store.getState().scaleStep).then((ok) => {
    if (ok) {
      store.setState((s) => ({ recentPicks: touchRecent(s.recentPicks, id) }));
    }
  });
}

export function useTool(): Tool {
  return store((s) => s.tool);
}

export function useScale(): i32 {
  return store((s) => s.scaleStep);
}

export function setDockMode(mode: DockMode): void {
  store.setState({ dockMode: mode });
}

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

/** A `delta` of 1 is one step coarser, and -1 one step finer. */
export function stepScale(delta: i32): void {
  const { tool, id, scaleStep } = store.getState();
  if (tool !== TOOL_ITEM || kindOf(id) !== KIND_CUBE) return;
  selectScale(Math.min(0, Math.max(1 - OBJECT_SCALE_CNT, scaleStep + delta)));
  flashSwatch(id, true);
}

/** The line joins the chat once the core takes it, and a line it refuses
 * whole does not. */
export function appendChatLine(text: string): void {
  scene.chat(text).then(
    () =>
      store.setState((s) => ({
        chatLines: [...s.chatLines, { id: nextChatLineId++, text }].slice(
          -MAX_CHAT_LINES,
        ),
      })),
    () => {},
  );
}

export function useInHand(id: AssetId): boolean {
  return store((s) => s.tool === TOOL_ITEM && s.id === id);
}
