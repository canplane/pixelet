// @pxl/web/app/settings/slices/lighting_slice.ts

import type { StateCreator } from "zustand";

/** Fields backing settings/sections/lighting_section.tsx. */
export type LightingSettingsSlice = {
  /**
   * Real-time shadow mapping (environment.tsx's sun/moon DirectionalLights
   * + chunk_stage.tsx's instanced meshes). Disabled by default: this is a
   * full second shadow-camera depth pass per casting light, even with
   * manual shadow.needsUpdate throttling instead of a per-frame refresh.
   */
  show_shadow_map: boolean;
  set_show_shadow_map: (show: boolean) => void;
  toggle_shadow_map: () => void;

  /**
   * Fresnel/rim highlight, applied uniformly across every chunk_stage.tsx
   * instance mesh — terrain and placed objects alike (see environment/
   * config.ts's rim_cfg). Just a uniform flip on an already-compiled shader
   * term, not a real extra pass — cheap enough to default on, unlike the
   * post-stack effects in post_fx_slice.ts.
   */
  show_rim_light: boolean;
  set_show_rim_light: (show: boolean) => void;
  toggle_rim_light: () => void;

  /**
   * GodRays light-shaft effect off the real-time sun. Needs a real occludable
   * scene Mesh as its light source (environment.tsx's sun_proxy_ref/
   * SUN_PROXY_DISTANCE — the sun otherwise only exists as a fragment-shader
   * disc drawn on the sky dome, not a real object anything could occlude),
   * plus its own dedicated screen-space raymarch pass. Disabled by default —
   * likely the single most expensive effect after real-time shadows.
   */
  show_god_rays: boolean;
  set_show_god_rays: (show: boolean) => void;
  toggle_god_rays: () => void;
};

export const create_lighting_settings_slice: StateCreator<LightingSettingsSlice, [], [], LightingSettingsSlice> = (set) => ({
  show_shadow_map: false,
  set_show_shadow_map: (show_shadow_map) => set({ show_shadow_map }),
  toggle_shadow_map: () => set((state) => ({ show_shadow_map: !state.show_shadow_map })),

  show_rim_light: true,
  set_show_rim_light: (show_rim_light) => set({ show_rim_light }),
  toggle_rim_light: () => set((state) => ({ show_rim_light: !state.show_rim_light })),

  show_god_rays: false,
  set_show_god_rays: (show_god_rays) => set({ show_god_rays }),
  toggle_god_rays: () => set((state) => ({ show_god_rays: !state.show_god_rays })),
});
