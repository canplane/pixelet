// @pxl/web/app/settings/settings_store.ts

import { create } from "zustand";
import { persist } from "zustand/middleware";

import { type CameraSettingsSlice, create_camera_settings_slice } from "./slices/camera_slice.ts";
import { type CharacterSettingsSlice, create_character_settings_slice } from "./slices/character_slice.ts";
import { create_lighting_settings_slice, type LightingSettingsSlice } from "./slices/lighting_slice.ts";
import { create_post_fx_settings_slice, type PostFxSettingsSlice } from "./slices/post_fx_slice.ts";
import { create_quality_settings_slice, type QualitySettingsSlice } from "./slices/quality_slice.ts";

/**
 * User-facing app settings — everything reachable through the Settings
 * modal (Character/Camera/Lighting/PostFx/Quality sections), whether that's
 * a real graphics option, a camera-interaction preference, or a companion
 * variant pick. One slice per section (slices/*.ts) so each section's own
 * fields stay in their own file, composed into this single persisted store
 * rather than one flat per-field list here. Dev-only diagnostics that
 * aren't a "setting" in this sense (the debug gizmo overlay, whether the
 * companion's models have finished loading) stay in their own separate
 * stores instead (debug_store.ts, characters/store.ts's
 * CompanionLoadStore) rather than a slice merged in here.
 */
export type SettingsStore =
  & CharacterSettingsSlice
  & CameraSettingsSlice
  & QualitySettingsSlice
  & LightingSettingsSlice
  & PostFxSettingsSlice;

/**
 * Which settings the user has actually changed. Only these are persisted.
 *
 * A default that has been written to storage is no longer a default: it is
 * a decision nobody made, and changing the value in the source then reaches
 * only people with empty storage. Keeping the untouched ones out means a
 * default stays what it claims to be — the fallback — and moves for
 * everyone who never had an opinion about it.
 *
 * Seeded from whatever was stored, since the stored keys are exactly the
 * ones a previous session had marked.
 */
const touched = new Set<string>();

type Patch = Partial<SettingsStore>;

/** Every setter in every slice goes through this, so no slice has to
 * remember to record anything. A write that changes nothing marks nothing —
 * some setters return the whole state to signal "no change", and that must
 * not read as the user having touched all of it. */
function mark_touched(patch: Patch, previous: SettingsStore): void {
  for (const [key, value] of Object.entries(patch)) {
    if (typeof value === "function") continue;
    if (!Object.is(value, (previous as Record<string, unknown>)[key])) touched.add(key);
  }
}

// Persisted (localStorage) — these are real user preferences (which
// effects to pay for, how the camera behaves, which companion to walk
// around with), not session-scoped state, so they should survive a reload
// instead of silently resetting to defaults every time.
export const useSettingsStore = create<SettingsStore>()(persist((set, get, api) => {
  const tracked_set: typeof set = (partial, replace) => {
    const previous = get();
    if (typeof partial === "function") {
      set((state) => {
        const patch = (partial as (state: SettingsStore) => Patch)(state);
        mark_touched(patch, previous);
        return patch;
      }, replace as false | undefined);
      return;
    }
    mark_touched(partial as Patch, previous);
    set(partial, replace as false | undefined);
  };

  return {
    ...create_character_settings_slice(tracked_set, get, api),
    ...create_camera_settings_slice(tracked_set, get, api),
    ...create_quality_settings_slice(tracked_set, get, api),
    ...create_lighting_settings_slice(tracked_set, get, api),
    ...create_post_fx_settings_slice(tracked_set, get, api),
  };
}, {
  name: "pxl:render_settings",
  partialize: (state) => Object.fromEntries(Object.entries(state).filter(([key]) => touched.has(key))) as Partial<SettingsStore>,
  merge: (persisted, current) => {
    for (const key of Object.keys((persisted ?? {}) as object)) touched.add(key);
    return { ...current, ...(persisted as object) };
  },
}));
