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

// Persisted (localStorage) — these are real user preferences (which
// effects to pay for, how the camera behaves, which companion to walk
// around with), not session-scoped state, so they should survive a reload
// instead of silently resetting to defaults every time.
export const useSettingsStore = create<SettingsStore>()(persist((...a) => ({
  ...create_character_settings_slice(...a),
  ...create_camera_settings_slice(...a),
  ...create_quality_settings_slice(...a),
  ...create_lighting_settings_slice(...a),
  ...create_post_fx_settings_slice(...a),
}), { name: "pxl:render_settings" }));
