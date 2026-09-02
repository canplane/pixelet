// @pxl/web/app/settings/slices/quality_slice.ts

import type { StateCreator } from "zustand";

/** Fields backing settings/sections/quality_section.tsx. */
export type QualitySettingsSlice = {
  /**
   * How this renderer fights edge/shading aliasing — mutually exclusive,
   * not three independent toggles, because "supersample" and "smaa" solve
   * the same visible problem by different means and stacking both wastes
   * GPU budget for no real gain (SMAA's edge-detection has almost nothing
   * left to find once supersampling has already spatially anti-aliased the
   * image; see scene.tsx's own dpr comment for why MSAA alone can't
   * substitute for either).
   *
   * - "none": no mitigation. Free.
   * - "smaa": post-process edge detection/blend (scene.tsx's SMAA pass).
   *   The only option that does anything on a device whose real
   *   devicePixelRatio is 1 — "supersample"'s [1,2] dpr clamp is a no-op
   *   there (clamp(1, 1, 2) = 1).
   * - "supersample": Canvas dpr [1,2] — real spatial supersampling, not
   *   just edge AA (fixes both edge aliasing and the shading-value flicker
   *   thousands of small flat-shaded boxes produce once they minify below
   *   a pixel). Costs roughly 4x the fragment-shading work of the whole
   *   pipeline on a Retina/high-DPI display, since dpr=2 means 4x the
   *   pixels — confirmed the single biggest post-stack cost on real
   *   devices, but the app ships with a real art pack behind it and
   *   defaults to this anyway. Only actually costs anything on a device
   *   whose real devicePixelRatio is already > 1.
   */
  aa_mode: "none" | "smaa" | "supersample";
  set_aa_mode: (mode: "none" | "smaa" | "supersample") => void;

  /**
   * Ground-color resolution mode (see chunk_stage.tsx's own u_low_spec
   * uniform). false (default) is real texture-pack sampling
   * (GROUND_LAYERS_TOP/SIDE, 4-tap supersampled) — more expensive (4 extra
   * sampler2DArray reads/instance), but what the app ships showing out of
   * the box now that a real art pack backs it. true is the flat
   * per-instance averaged color instead — cheapest, a fallback for
   * low-end devices rather than the default.
   */
  low_spec: boolean;
  set_low_spec: (low_spec: boolean) => void;
  toggle_low_spec: () => void;
};

export const create_quality_settings_slice: StateCreator<QualitySettingsSlice, [], [], QualitySettingsSlice> = (set) => ({
  aa_mode: "supersample",
  set_aa_mode: (aa_mode) => set({ aa_mode }),

  low_spec: false,
  set_low_spec: (low_spec) => set({ low_spec }),
  toggle_low_spec: () => set((state) => ({ low_spec: !state.low_spec })),
});
