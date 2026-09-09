// @pxl/web/app/settings/slices/camera_slice.ts

import type { StateCreator } from "zustand";

export type CameraTerrainMode = "through" | "stop" | "climb";

/** Fields backing settings/sections/camera_section.tsx. */
export type CameraSettingsSlice = {
  /**
   * When true, camera/input.tsx's rotate-drag and pinch gestures no longer
   * write polar (tilt) — azimuth still works exactly the same either way.
   * The gesture code itself (axis-lock, pinch tracking) is untouched by
   * this; the flag only gates the final set_view_state call each path
   * would otherwise make. Defaults to true — free-tilt-by-gesture was
   * validated (2026-08-30) as UX complexity not worth keeping on by
   * default, not a bug fix, which is why this is a toggle here rather
   * than a deleted code path: easy to flip back on to compare against.
   */
  lock_camera_tilt: boolean;
  set_lock_camera_tilt: (lock: boolean) => void;
  toggle_lock_camera_tilt: () => void;

  /** When enabled, panning re-samples terrain elevation under the map center every drag step. */
  ground_snap: boolean;
  set_ground_snap: (ground_snap: boolean) => void;
  toggle_ground_snap: () => void;

  /**
   * Whether the terrain shader paints the STRUCTURE ground channel, i.e.
   * whether OSM buildings are drawn. The bake writes that channel either way
   * (see osm_policy.ts's determine_cell_cfg), so this is purely a render
   * decision. Still false by default; whether that is the right default is
   * a separate question from where the switch lives.
   */
  show_structures: boolean;
  set_show_structures: (show: boolean) => void;
  toggle_structures: () => void;

  /**
   * Whether the eraser tool's "nothing placed here" case (camera/input.tsx's
   * handle_tap) digs into the terrain's own real surface (place_air) instead
   * of no-op'ing the way it always did before this flag existed. Off by
   * default: the terrain here is real, baked map data (OSM-derived), not a
   * user's own scratch canvas the way placed objects are — carving into it
   * is a deliberate, opt-in mode, not eraser's default behavior.
   */
  enable_terrain_carving: boolean;
  set_enable_terrain_carving: (enable: boolean) => void;
  toggle_terrain_carving: () => void;

  /** Whether cube edges bordering open air are darkened, so flat ground
   * stays seamless and height changes read as a line
   * (world/render/chunk_material.ts). Strength and width stay dev knobs. */
  show_edge_shade: boolean;
  set_show_edge_shade: (show: boolean) => void;
  toggle_edge_shade: () => void;

  /** Whether procedurally scattered decor is drawn. User-placed decor is
   * unaffected — that is the player's own content, like a placed cube. */
  show_decor: boolean;
  set_show_decor: (show: boolean) => void;
  toggle_show_decor: () => void;

  /**
   * What terrain does to the camera (camera/terrain_gate.ts).
   *
   *   through — nothing. The eye goes wherever the controls put it, ground
   *             included, where the terrain shell has no inward-facing
   *             geometry and the scene reads inside out.
   *   stop    — a control goes as far as it asked or as far as it can.
   *             Nothing moves on its own; a zoom into a hill stops short.
   *   climb   — dist is always granted and the *view* pitches up instead,
   *             as far as straight down, so every zoom level stays
   *             reachable at the cost of an angle nobody asked for.
   */
  camera_terrain: CameraTerrainMode;
  set_camera_terrain: (mode: CameraTerrainMode) => void;

  /** Whether poi_layer.tsx's PoiLayer renders anything at all — a plain
   * visibility switch on lightweight billboard overlays, not a rendering-
   * cost tradeoff the way show_structures is (that one gates a baked
   * mesh channel), so it needs no deeper wiring than an early return.
   * Defaults to true: POI badges are core to reading the map, not an
   * opt-in extra. */
  show_poi: boolean;
  set_show_poi: (show: boolean) => void;
  toggle_show_poi: () => void;
};

export const create_camera_settings_slice: StateCreator<CameraSettingsSlice, [], [], CameraSettingsSlice> = (set) => ({
  lock_camera_tilt: true,
  set_lock_camera_tilt: (lock_camera_tilt) => set({ lock_camera_tilt }),
  toggle_lock_camera_tilt: () => set((state) => ({ lock_camera_tilt: !state.lock_camera_tilt })),

  ground_snap: true,
  set_ground_snap: (ground_snap) => set((state) => state.ground_snap === ground_snap ? state : { ground_snap }),
  toggle_ground_snap: () => set((state) => ({ ground_snap: !state.ground_snap })),

  show_structures: false,
  set_show_structures: (show_structures) => set({ show_structures }),
  toggle_structures: () => set((state) => ({ show_structures: !state.show_structures })),

  enable_terrain_carving: false,
  set_enable_terrain_carving: (enable_terrain_carving) => set({ enable_terrain_carving }),
  toggle_terrain_carving: () => set((state) => ({ enable_terrain_carving: !state.enable_terrain_carving })),

  show_edge_shade: true,
  set_show_edge_shade: (show_edge_shade) => set({ show_edge_shade }),
  toggle_edge_shade: () => set((state) => ({ show_edge_shade: !state.show_edge_shade })),

  show_decor: true,
  set_show_decor: (show_decor) => set({ show_decor }),
  toggle_show_decor: () => set((state) => ({ show_decor: !state.show_decor })),

  camera_terrain: "stop",
  set_camera_terrain: (camera_terrain) => set({ camera_terrain }),

  show_poi: true,
  set_show_poi: (show_poi) => set({ show_poi }),
  toggle_show_poi: () => set((state) => ({ show_poi: !state.show_poi })),
});
