// @pxl/web/render/settings_store.ts

import { create } from "zustand";
import { persist } from "zustand/middleware";

/**
 * User-facing rendering-quality settings — real graphics options, not dev
 * diagnostics (see debug_store.ts for those). Split out of debug_store.ts
 * because these are meant to stay reachable in production through the
 * Settings modal's Lighting/PostFx/Quality sections, unlike debug_store.ts's
 * own state.
 */
export type RenderSettingsStore = {
  /**
   * Ground-color resolution mode (see chunk_stage.tsx's own u_low_spec
   * uniform). false (default) is real texture-pack sampling
   * (GROUND_LAYERS_TOP/SIDE, 4-tap supersampled) — more expensive (4 extra
   * sampler2DArray reads/instance), but what the app ships showing out of
   * the box now that a real art pack ("cozy") backs it. true is the flat
   * per-instance averaged color instead — cheapest, a fallback for
   * low-end devices rather than the default.
   */
  low_spec: boolean;
  set_low_spec: (low_spec: boolean) => void;
  toggle_low_spec: () => void;

  /**
   * Whether the terrain shader paints the STRUCTURE ground channel, i.e.
   * whether OSM buildings are drawn. The bake writes that channel either way
   * (see osm_policy.ts's determine_cell_cfg), so this is purely a render
   * decision.
   *
   * Belongs here rather than app/dev/debug_store.ts because that store's
   * toggles are __IS_DEV__-gated — living there would make buildings
   * unreachable in a production build for a setting that is a visual
   * feature with a cost, not a diagnostic. Still false by default; whether
   * that is the right default is a separate question from where the switch
   * lives.
   */
  show_structures: boolean;
  set_show_structures: (show: boolean) => void;
  toggle_structures: () => void;

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

  /** SMAA post-processing anti-aliasing. Disabled by default — same post-stack cost note as show_ao. */
  show_smaa: boolean;
  set_show_smaa: (show: boolean) => void;
  toggle_smaa: () => void;

  /** ChromaticAberration — subtle RG/B channel split toward the frame edge, a cheap lens-fringing cue for the miniature-diorama look. Disabled by default like the rest of this store's post-stack toggles, but on the "not a vanilla Minecraft option" mobile-default principle (see scene.tsx's own comment) — unlike show_ao/show_bloom/show_tilt_shift/show_smaa, this one was never itself profiled. */
  show_chromatic_aberration: boolean;
  set_show_chromatic_aberration: (show: boolean) => void;
  toggle_chromatic_aberration: () => void;

  /**
   * GodRays light-shaft effect off the real-time sun. Needs a real occludable
   * scene Mesh as its light source (environment.tsx's sun_proxy_ref/
   * SUN_PROXY_DISTANCE — the sun otherwise only exists as a fragment-shader
   * disc drawn on the sky dome, not a real object anything could occlude),
   * plus its own dedicated screen-space raymarch pass. Disabled by default —
   * same post-stack cost note as show_ao, likely pricier given the extra
   * light-source render pass GodRaysEffect needs on top of the raymarch itself.
   */
  show_god_rays: boolean;
  set_show_god_rays: (show: boolean) => void;
  toggle_god_rays: () => void;

  /**
   * Fresnel/rim highlight on placed objects (chunk_stage.tsx's OBJECT_BOX
   * instances only, never terrain — see environment/config.ts's rim_cfg).
   * Just a uniform flip on an already-compiled shader term, not a real
   * extra pass — cheap enough to default on, unlike the post-stack effects
   * above.
   */
  show_rim_light: boolean;
  set_show_rim_light: (show: boolean) => void;
  toggle_rim_light: () => void;

  /**
   * Canvas dpr cap (scene.tsx). false (default) pins dpr to [1,1] — the
   * single biggest post-stack cost, confirmed on real devices. true opts
   * into [1,2]: real spatial supersampling (fixes both edge aliasing and
   * the shading-value flicker thousands of small flat-shaded boxes produce
   * once they minify below a pixel). Costs roughly 4x the fragment-shading
   * work of the whole pipeline on a Retina/high-DPI display, since dpr=2
   * means 4x the pixels.
   */
  high_dpr: boolean;
  set_high_dpr: (show: boolean) => void;
  toggle_high_dpr: () => void;

  /**
   * Soft radial-gradient decal under each placed solid object, faking
   * ground contact without any lighting/depth computation — no post pass,
   * no shadow map, just an alpha-blended flat quad per object (see
   * modules/world/objects/blob_shadow.tsx). Cheap enough to ship on by
   * default eventually, but defaults off for now — the look still reads
   * oddly (2026-08-15 review) even after fixing the corner-vs-center
   * position bug, needs another pass before it's worth shipping visible.
   */
  show_blob_shadow: boolean;
  set_show_blob_shadow: (show: boolean) => void;
  toggle_blob_shadow: () => void;

  /**
   * Real-time shadow mapping (environment.tsx's sun/moon DirectionalLights
   * + chunk_stage.tsx's instanced meshes). Disabled by default: this is the
   * single most expensive effect in this store, a full second
   * shadow-camera depth pass per casting light, even with manual
   * shadow.needsUpdate throttling instead of a per-frame refresh.
   */
  show_shadow_map: boolean;
  set_show_shadow_map: (show: boolean) => void;
  toggle_shadow_map: () => void;
};

// Persisted (localStorage) — these are real user preferences (which
// effects to pay for), not session-scoped state, so they should survive a
// reload instead of silently resetting to defaults every time.
export const useRenderSettingsStore = create<RenderSettingsStore>()(persist((set) => ({
  low_spec: false,
  set_low_spec: (low_spec) => set({ low_spec }),
  toggle_low_spec: () => set((state) => ({ low_spec: !state.low_spec })),

  show_structures: false,
  set_show_structures: (show_structures) => set({ show_structures }),
  toggle_structures: () => set((state) => ({ show_structures: !state.show_structures })),

  show_ao: false,
  set_show_ao: (show_ao) => set({ show_ao }),
  toggle_ao: () => set((state) => ({ show_ao: !state.show_ao })),

  show_bloom: false,
  set_show_bloom: (show_bloom) => set({ show_bloom }),
  toggle_bloom: () => set((state) => ({ show_bloom: !state.show_bloom })),

  show_tilt_shift: false,
  set_show_tilt_shift: (show_tilt_shift) => set({ show_tilt_shift }),
  toggle_tilt_shift: () => set((state) => ({ show_tilt_shift: !state.show_tilt_shift })),

  show_smaa: false,
  set_show_smaa: (show_smaa) => set({ show_smaa }),
  toggle_smaa: () => set((state) => ({ show_smaa: !state.show_smaa })),

  show_chromatic_aberration: false,
  set_show_chromatic_aberration: (show_chromatic_aberration) => set({ show_chromatic_aberration }),
  toggle_chromatic_aberration: () => set((state) => ({ show_chromatic_aberration: !state.show_chromatic_aberration })),

  show_god_rays: false,
  set_show_god_rays: (show_god_rays) => set({ show_god_rays }),
  toggle_god_rays: () => set((state) => ({ show_god_rays: !state.show_god_rays })),

  show_rim_light: true,
  set_show_rim_light: (show_rim_light) => set({ show_rim_light }),
  toggle_rim_light: () => set((state) => ({ show_rim_light: !state.show_rim_light })),

  high_dpr: false,
  set_high_dpr: (high_dpr) => set({ high_dpr }),
  toggle_high_dpr: () => set((state) => ({ high_dpr: !state.high_dpr })),

  show_blob_shadow: false,
  set_show_blob_shadow: (show_blob_shadow) => set({ show_blob_shadow }),
  toggle_blob_shadow: () => set((state) => ({ show_blob_shadow: !state.show_blob_shadow })),

  show_shadow_map: false,
  set_show_shadow_map: (show_shadow_map) => set({ show_shadow_map }),
  toggle_shadow_map: () => set((state) => ({ show_shadow_map: !state.show_shadow_map })),
}), { name: "pxl:render_settings" }));
