// @pxl/web/app/dev/debug_store.ts

import { create } from "zustand";

export type DebugStore = {
  /**
   * Whether the dev panel's secondary buttons (건물, 더미 토스트, the
   * 3D-gizmo toggle below, ...) are revealed at all — the "debug" button
   * itself. Previously this same flag also gated the 3D debug gizmos
   * directly, conflating "is the dev menu open" with "are the gizmos
   * drawn": opening the menu to reach an unrelated button (e.g. spawning
   * dummy toasts) always dragged the gizmos on screen too, with no way to
   * have one without the other.
   */
  show_debug_menu: boolean;
  set_show_debug_menu: (show: boolean) => void;
  toggle_debug_menu: () => void;

  /**
   * Runtime toggle for the dev-only debug gizmos (frame-origin spheres, the
   * map_center/map_origin dots, page-corner blocks — everything gated by
   * `__IS_DEV__` in world.tsx's DebugLayer). `__IS_DEV__` alone used to be
   * the only gate, meaning every dev build always rendered them, cluttering
   * the exact area (near the device's own position) the user marker and
   * terrain live in — no way to turn them off short of a prod build.
   * Defaults to false: these are opt-in diagnostics, not something that
   * should be in the way while eyeballing real rendering behavior. Its own
   * button only surfaces once show_debug_menu is on (see dev_panel.tsx).
   */
  show_debug_objects: boolean;
  set_show_debug_objects: (show: boolean) => void;
  toggle_debug_objects: () => void;

  /**
   * Terrain bakes buildings into the STRUCTURE ground channel regardless of
   * this flag (see osm_policy.ts's determine_cell_cfg) — this only controls
   * whether the terrain shader paints that channel (shader.ts's
   * u_show_structures / is_structure). Defaults to false: structures are
   * hidden out of the box, same as the other debug-only visuals here, and
   * only surface via dev_panel.tsx's own toggle button once debug mode
   * itself is on.
   */
  show_structures: boolean;
  set_show_structures: (show: boolean) => void;
  toggle_structures: () => void;

  /**
   * Ground-color resolution mode (see chunk_stage.tsx's own u_low_spec
   * uniform / inject_instance_rgb): true (default) is the flat per-instance
   * averaged color — cheapest, and what the app ships showing out of the
   * box. false opts into real texture-pack sampling (GROUND_LAYERS_TOP/
   * SIDE, 4-tap supersampled) instead — more expensive (4 extra
   * sampler2DArray reads/instance) and dev-panel-only for now, toggled via
   * the "텍스처팩" button once show_debug_menu is on. Predates the unified
   * ChunkStage pipeline (used to gate the OLD terrain shader's own
   * vertex-vs-fragment resolution split, a different mechanism this
   * pipeline has no equivalent of) — kept the same store field/button
   * rather than adding a second one, since "flat color vs. real texture" is
   * still the same user-facing choice either way.
   */
  low_spec: boolean;
  set_low_spec: (low_spec: boolean) => void;
  toggle_low_spec: () => void;
};

export const useDebugStore = create<DebugStore>((set) => ({
  show_debug_menu: false,
  set_show_debug_menu: (show_debug_menu) => set({ show_debug_menu }),
  toggle_debug_menu: () => set((state) => ({ show_debug_menu: !state.show_debug_menu })),

  show_debug_objects: false,
  set_show_debug_objects: (show_debug_objects) => set({ show_debug_objects }),
  toggle_debug_objects: () => set((state) => ({ show_debug_objects: !state.show_debug_objects })),

  show_structures: false,
  set_show_structures: (show_structures) => set({ show_structures }),
  toggle_structures: () => set((state) => ({ show_structures: !state.show_structures })),

  low_spec: true,
  set_low_spec: (low_spec) => set({ low_spec }),
  toggle_low_spec: () => set((state) => ({ low_spec: !state.low_spec })),
}));
