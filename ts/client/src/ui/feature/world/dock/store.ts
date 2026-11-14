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
  kind_of,
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
let next_chat_line_id = 0;

// A saved hotbar may come from another build, with ids this catalog lacks.
function is_known_pick(id: AssetId): boolean {
  return id === MARKER || DECORS.includes(id) || CUBES.includes(id);
}

function touch_recent(
  recent_picks: readonly AssetId[],
  id: AssetId,
): AssetId[] {
  if (recent_picks.includes(id)) return recent_picks as AssetId[];
  return [id, ...recent_picks].slice(0, RECENT_PICKS);
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
  scale_step: i32;
  // The hotbar, newest asset first; picking one already on it moves nothing.
  // Only `select_asset` adds a pick, and only once the core holds it.
  recent_picks: AssetId[];
  chat_lines: ChatLine[];
  dock_mode: DockMode;
  /** A pick or a size step made with a key. The dock shows it over the cube's
   * swatch as a press would (popover.tsx). */
  swatch_flash: SwatchFlash;
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
        .filter((p) => typeof p === "number" && is_known_pick(p))
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
    JSON.stringify({ hotbar: s.recent_picks } satisfies Stored),
  );
}

const store = create<DockState>()(() => ({
  tool: TOOL_POINTER,
  id: 0,
  scale_step: 0,
  recent_picks: load().hotbar,
  chat_lines: [],
  dock_mode: DockMode.PALETTE,
  swatch_flash: { n: 0, cube: null, open: false },
}));

store.subscribe((s, previous) => {
  if (s.recent_picks !== previous.recent_picks) save(s);
});

export function useRecentPicks(): readonly AssetId[] {
  return store((s) => s.recent_picks);
}

export function useChatLines(): readonly ChatLine[] {
  return store((s) => s.chat_lines);
}

export function useDockMode(): DockMode {
  return store((s) => s.dock_mode);
}

export function useSwatchFlash(): SwatchFlash {
  return store((s) => s.swatch_flash);
}

export function recent_pick(slot: number): AssetId {
  return store.getState().recent_picks[slot];
}

export function dock_mode(): DockMode {
  return store.getState().dock_mode;
}

export function watch_dock_mode(
  listener: (mode: DockMode, before: DockMode) => void,
): () => void {
  return store.subscribe((s, previous) => {
    if (s.dock_mode !== previous.dock_mode)
      listener(s.dock_mode, previous.dock_mode);
  });
}

/** Tells the core what is held, and keeps it only if the core accepts it, so
 * that the dock never shows a tool the core refused. */
async function hold(
  tool: Tool,
  id: AssetId,
  scale_step: i32,
): Promise<boolean> {
  try {
    await edit.tool(tool, id, scale_step);
  } catch (err) {
    console.error(`[dock] the core refused the tool: ${String(err)}`);
    return false;
  }
  store.setState({ tool, id, scale_step });
  return true;
}

export function select_tool(tool: Tool): void {
  const { id, scale_step } = store.getState();
  void hold(tool, id, scale_step);
}

export function select_scale(scale_step: i32): void {
  const { tool, id } = store.getState();
  void hold(tool, id, scale_step);
}

export function select_asset(id: AssetId): void {
  void hold(TOOL_ITEM, id, store.getState().scale_step).then((ok) => {
    if (ok) {
      store.setState((s) => ({
        recent_picks: touch_recent(s.recent_picks, id),
      }));
    }
  });
}

export function useTool(): Tool {
  return store((s) => s.tool);
}

export function useScale(): i32 {
  return store((s) => s.scale_step);
}

export function set_dock_mode(mode: DockMode): void {
  store.setState({ dock_mode: mode });
}

export function switch_dock_mode(): void {
  set_dock_mode(
    dock_mode() === DockMode.MESSAGE ? DockMode.PALETTE : DockMode.MESSAGE,
  );
}

export function flash_swatch(cube: Cube, open: boolean): void {
  store.setState((s) => ({
    swatch_flash: { n: s.swatch_flash.n + 1, cube, open },
  }));
}

/** A `delta` of 1 is one step coarser, and -1 one step finer. */
export function step_scale(delta: i32): void {
  const { tool, id, scale_step } = store.getState();
  if (tool !== TOOL_ITEM || kind_of(id) !== KIND_CUBE) return;
  select_scale(Math.min(0, Math.max(1 - OBJECT_SCALE_CNT, scale_step + delta)));
  flash_swatch(id, true);
}

/** The line joins the chat once the core takes it, and a line it refuses
 * whole does not. */
export function append_chat_line(text: string): void {
  scene.chat(text).then(
    () =>
      store.setState((s) => ({
        chat_lines: [...s.chat_lines, { id: next_chat_line_id++, text }].slice(
          -MAX_CHAT_LINES,
        ),
      })),
    () => {},
  );
}

export function useInHand(id: AssetId): boolean {
  return store((s) => s.tool === TOOL_ITEM && s.id === id);
}
