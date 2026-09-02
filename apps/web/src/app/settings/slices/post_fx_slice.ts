// @pxl/web/app/settings/slices/post_fx_slice.ts

import type { StateCreator } from "zustand";

/** Fields backing settings/sections/post_fx_section.tsx. */
export type PostFxSettingsSlice = {
  /**
   * N8AO (screen-space contact/crevice ambient occlusion). Disabled by
   * default: confirmed real cost via live profiling (WebGLRenderer.
   * renderBufferDirect/setProgram time scaling with the post stack — see
   * scene.tsx's own comment).
   */
  show_ao: boolean;
  set_show_ao: (show: boolean) => void;
  toggle_ao: () => void;

  /** Bloom glow on very bright pixels (sun/moon disc, the night location beacon). Disabled by default — same post-stack cost note as show_ao. */
  show_bloom: boolean;
  set_show_bloom: (show: boolean) => void;
  toggle_bloom: () => void;

  /** TiltShift2 screen-space blur band — the "miniature diorama" lens look. Disabled by default — same post-stack cost note as show_ao. */
  show_tilt_shift: boolean;
  set_show_tilt_shift: (show: boolean) => void;
  toggle_tilt_shift: () => void;

  /** ChromaticAberration — subtle RG/B channel split toward the frame edge, a cheap lens-fringing cue for the miniature-diorama look. Disabled by default like the rest of this slice's post-stack toggles, but on the "not a vanilla Minecraft option" mobile-default principle (see scene.tsx's own comment) — unlike show_ao/show_bloom/show_tilt_shift, this one was never itself profiled. */
  show_chromatic_aberration: boolean;
  set_show_chromatic_aberration: (show: boolean) => void;
  toggle_chromatic_aberration: () => void;
};

export const create_post_fx_settings_slice: StateCreator<PostFxSettingsSlice, [], [], PostFxSettingsSlice> = (set) => ({
  show_ao: false,
  set_show_ao: (show_ao) => set({ show_ao }),
  toggle_ao: () => set((state) => ({ show_ao: !state.show_ao })),

  show_bloom: false,
  set_show_bloom: (show_bloom) => set({ show_bloom }),
  toggle_bloom: () => set((state) => ({ show_bloom: !state.show_bloom })),

  show_tilt_shift: false,
  set_show_tilt_shift: (show_tilt_shift) => set({ show_tilt_shift }),
  toggle_tilt_shift: () => set((state) => ({ show_tilt_shift: !state.show_tilt_shift })),

  show_chromatic_aberration: false,
  set_show_chromatic_aberration: (show_chromatic_aberration) => set({ show_chromatic_aberration }),
  toggle_chromatic_aberration: () => set((state) => ({ show_chromatic_aberration: !state.show_chromatic_aberration })),
});
