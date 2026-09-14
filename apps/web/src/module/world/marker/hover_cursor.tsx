// @pxl/web/module/world/marker/hover_cursor.tsx

import { useEffect, useMemo } from "react";

import type { PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { U_PER_CELL } from "@pxl/shared/bake/constant.ts";

import { useCameraStore } from "@/module/camera/store.ts";
import { useUiStore } from "@/app/store/ui_store.ts";
import { useMapStore } from "@/module/map/store.ts";
import { useSettingsStore } from "@/app/setting/store.ts";
import { get_local_world_origin } from "../frame/coord.ts";
import { is_placement_blocked } from "../object/collision.ts";
import { DECOR_SIZE } from "../object/size.ts";
import { clear_virtual_carve_preview, set_virtual_carve_preview, set_virtual_hide, usePlacedObject } from "../object/store.ts";
import type { PlaceableCubeId } from "@pxl/shared/spec/cell/mod.ts";
import { user_decor_pick } from "../decor/decor_kinds.ts";
import { decor_bundle_for } from "../decor/decor_registry.ts";
import { decor_assets_for, useDecorAssets } from "../decor/asset_store.ts";
import { marker_colors } from "./style.ts";
import { shown_ground } from "@pxl/shared/bake/ground.ts";
import { placeable_cube_hex, placeable_cube_num_id, terrain_style_cube_for } from "@pxl/shared/spec/cell/mod.ts";
import { get_ground_type } from "../terrain/elevation.ts";
import { fvec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { CORE_PXL } from "@/wasm/core_pool.ts";
import { DECOR_BUNDLES } from "../decor/decor_registry.ts";
import { request_world_tick } from "../change.ts";

// The ghost is the core's (crates/pxl-core/src/world/ghost.rs): a box written in
// the mesher's layout and drawn by the chunk shader, or one decor instance
// drawn by decor's, half transparent. Everything here decides which ghost
// and where.

// Stable empty reference for the renders where no decor bundle applies, so
// nothing downstream sees a fresh `[]` identity every frame.
const EMPTY_ASSETS: readonly null[] = [];

export function HoverCursor() {
  const hovered: PVF3 | null = useCameraStore((state) => state.hovered_coord);
  const hovered_object_id = useCameraStore((state) => state.hovered_object_id);
  const activeTool = useUiStore((state) => state.activeTool);
  const object_color_id = useUiStore((state) => state.object_color_id);
  const object_size = useUiStore((state) => state.object_size);
  const ui_decor_id = useUiStore((state) => state.decor_id);
  const map_center = useMapStore((state) => state.map_center);
  const enable_terrain_carving = useSettingsStore((state) => state.enable_terrain_carving);

  const hovered_object = usePlacedObject(hovered_object_id);

  // Eraser's own "dig into the terrain itself" case (camera/input.tsx's
  // handle_tap/update_hover) — gated on the same settings flag those use, so
  // this ghost never previews an edit the tap handler wouldn't actually
  // commit. With the flag off, eraser hovering bare terrain falls through to
  // exactly the same blocked/no-ghost treatment it always had before this
  // feature existed (see `blocked` below).
  const digging_terrain = activeTool === "eraser" && !hovered_object && enable_terrain_carving;
  const show_structures = useSettingsStore((state) => state.show_structures);
  const texture_pack = useSettingsStore((state) => state.texture_pack);

  const use_object_style = activeTool === "object" || activeTool === "decor" || activeTool === "eraser" || hovered_object !== null;

  // Eraser sizes to whatever's actually hovered when there's a real object
  // under the cursor (about to be removed at its own size) but falls
  // through to object_size, like the object tool, over bare terrain — its
  // "dig into the terrain itself" case has no fixed size of its own, it
  // shares the same tier picker object placement uses (mode_bar.tsx's size
  // flyout).
  const preview_size = (activeTool === "eraser" || activeTool === "pointer") && hovered_object ? hovered_object.size : activeTool === "decor" ? DECOR_SIZE : object_size;

  // The ghost box fills [anchor, anchor + current_scale] on every axis — one
  // cell for a bare-terrain hover (world_hover_pvf3's anchor is the struck
  // voxel's own min corner now, see camera/ray.ts), the placement/erase size
  // otherwise. No separate terrain-vs-object offset: every anchor this file
  // sees is a min corner, so the box always sits at + current_scale/2.
  const current_scale = use_object_style ? preview_size : U_PER_CELL;
  // Same "object size vs decor's own fixed DECOR_SIZE" dispatch
  // camera/input.tsx's own handle_tap needs to actually commit a placement —
  // object/collision.ts's own is_placement_blocked is the one shared place
  // that decides it, so the preview here and the real commit there can't
  // silently disagree about what counts as blocked.
  const blocked = useMemo(() => {
    if (!hovered) return false;
    if (activeTool === "object" || activeTool === "decor") {
      const map_origin = get_local_world_origin(map_center);
      return is_placement_blocked(activeTool, hovered, object_size, map_origin);
    }
    // Eraser hovering an existing object is never blocked (removing it is
    // always valid). Bare terrain is blocked exactly when digging into it
    // isn't even enabled (enable_terrain_carving off) — the tap handler
    // would no-op there too, so there's nothing this ghost should preview.
    if (activeTool === "eraser") return hovered_object === null && !enable_terrain_carving;
    return false;
  }, [activeTool, hovered, map_center, object_size, hovered_object, enable_terrain_carving]);

  // Makes the real erase candidate actually stop rendering (see
  // set_virtual_hide's own doc comment) — the ghost mesh below then reads as
  // a translucent preview over whatever's really behind/below it, not a
  // ghost merely laid on top of a still fully-opaque real object. Depends on
  // the plain id, not `hovered_object` itself, so an unrelated re-derivation
  // of the same cell (same id, new object literal) doesn't spuriously
  // clear-then-reset the hide and force a needless chunk rebuild.
  //
  // This ghost's own appearance/disappearance is instant, and the real
  // object's isn't (its own hide has to propagate through a few React
  // commits via chunk_stage.tsx's Chunk) — accepted as-is, a deliberate
  // choice, not an oversight: see set_virtual_hide's own doc comment for
  // the fade/multi-slot system that was built to mask the resulting few
  // milliseconds of overlap, then reverted for being real added complexity
  // for a barely-visible cosmetic gap.
  const virtual_hide_target = activeTool === "eraser" && hovered_object ? hovered_object.id : null;
  useEffect(() => {
    set_virtual_hide(virtual_hide_target);
    return () => set_virtual_hide(null);
  }, [virtual_hide_target]);

  // Terrain's own counterpart to virtual_hide_target above — eraser hovering
  // bare terrain has no real leaf to hide (see place_air's own doc comment),
  // so set_virtual_carve_preview injects a real AIR entry into the render
  // build instead, at this exact hover, so whatever's really one voxel below
  // shows through the ghost the same way an object's own hidden self does —
  // see that function's own doc comment for why a plain tinted overlay on
  // the still-opaque real terrain doesn't read as transparent at all.
  //
  // Depends on hovered's own i/f fields directly, not the hovered object
  // itself — world_struck_corner already snaps to the struck cell's own
  // grid, so two hover ticks landing in the same cell produce bit-identical
  // BigInts and this effect correctly skips re-running; depending on
  // `hovered` (a fresh PVF3 literal every tick) instead would re-trigger the
  // cleanup/re-set pair on every mouse-move frame regardless of whether the
  // cell actually changed.
  const carve_preview_active = digging_terrain && hovered !== null;
  useEffect(() => {
    if (carve_preview_active && hovered) set_virtual_carve_preview(hovered.i, hovered.f, preview_size);
    else clear_virtual_carve_preview();
    return () => clear_virtual_carve_preview();
  }, [carve_preview_active, hovered?.i, hovered?.f, preview_size]);

  // Pointer (select/move) always reads as the generic marker color, whether
  // it's hovering bare terrain or an existing object — the two need to look
  // identical (same cyan/sky-blue ghost) since neither is "here's the real
  // thing you're about to add or remove", just "here's what you could grab".
  // Object placement and eraser-on-an-existing-object instead ghost at the
  // real thing's own true color_id — the placement candidate's for object
  // mode, the existing object's own for eraser — since both of those really
  // are "here's the real thing". Eraser hovering bare terrain is the one
  // exception: there's no existing "real thing" to borrow a color from
  // (it's about to dig into the terrain itself, not remove something placed
  // on it), so that case falls through to null here and gets its own VOID
  // marker color below instead. decor_id-carrying cases (decor mode, or an
  // eraser hovering a decor entry) never reach this — decor_model below
  // gives them their own real silhouette+texture instead of a color swatch.
  //
  // Carving bare terrain resolves the same way. What the eraser removes
  // there is a terrain voxel, and the cube that looks like one is exactly
  // its terrain-styled id, so the two halves of the tool preview the thing
  // they are about to remove rather than one borrowing a colour and the
  // other a flat marker.
  const carved_style_id: PlaceableCubeId | null = useMemo(() => {
    if (!digging_terrain || !hovered) return null;
    const shown = shown_ground((ch) => get_ground_type(hovered.i, hovered.f, ch) ?? 0, show_structures);
    return terrain_style_cube_for(shown.channel, shown.type_id);
  }, [digging_terrain, hovered?.i, hovered?.f, show_structures]);

  const color_id: PlaceableCubeId | null = activeTool === "pointer" || activeTool === "decor"
    ? null
    : hovered_object?.kind === "solid"
    ? hovered_object.color_id
    : activeTool === "object"
    ? object_color_id
    : carved_style_id;

  // Which decor kind's own real silhouette this ghost should show, if any —
  // the kind about to be placed (decor tool active) or the kind already
  // sitting where an eraser/pointer hover landed. Read from the same store
  // the chunks draw from, so the ghost is the real model, loaded once.
  const decor_id = activeTool === "decor" ? ui_decor_id : hovered_object?.kind === "decor" ? hovered_object.decor_id : null;
  const decor_bundle = decor_id ? decor_bundle_for(decor_id) : null;
  const decor_assets = decor_assets_for(decor_bundle, useDecorAssets());
  const decor_models = decor_assets?.models ?? EMPTY_ASSETS;
  const decor_skins = decor_assets?.skins ?? EMPTY_ASSETS;

  // The exact `i` a real placement/hover resolves to — user_decor_pick is
  // pure and keyed only on this, so calling it here predicts (for a
  // placement candidate) or recovers (for an already-placed entry) the
  // real variant/heading a real render would use, not an arbitrary stand-in.
  const decor_pick_i = activeTool === "decor" ? hovered?.i ?? null : hovered_object?.kind === "decor" ? hovered_object.i : null;
  const decor_pick = decor_bundle && decor_pick_i !== null ? user_decor_pick(decor_pick_i, decor_bundle.variants.length) : null;
  const decor_model = decor_pick ? decor_models[decor_pick.variant] ?? null : null;
  const decor_skin = decor_pick ? decor_skins[decor_pick.variant] ?? null : null;
  // Both must be ready — an untextured white silhouette (model loaded, skin
  // not yet) would read as a real rendering glitch, not a loading state, so
  // this falls back to the flat box ghost until both resolve.
  const is_decor_ghost = decor_model !== null && decor_skin !== null;

  // Without a texture the ghost falls back to the thing's own swatch, which
  // is the same question for a placed cube and for terrain — whether the
  // voxel under the cursor was painted by the object layer or baked into
  // the ground is not a distinction the tool is about, so it must not be
  // one the preview shows.
  //
  // Nothing resolves a swatch only when the eraser is over decor, whose
  // silhouette is the real preview and whose model has not arrived yet.
  // That box takes ERASE, because the tool is what the user has to be able
  // to read off it, and ACTION's cyan says "move". ACTION stays for the
  // ghost with no change behind it at all: the pointer.
  const swatch = color_id ? placeable_cube_hex(color_id) : null;
  const fallback = activeTool === "eraser" ? marker_colors.ERASE : marker_colors.ACTION;

  // Whether this ghost sits flush/coincident with real, already-rendered
  // solid geometry rather than floating clear of anything else — decides
  // which of the two polygon-offset/renderOrder biases below actually wins
  // the depth test, independently of current_scale / where the box sits.
  // Pointer/no-candidate-context's own bare-terrain ghost (use_object_style
  // === false) is this case — a normal "object mode" ghost (a placement
  // candidate, or an existing object eraser is about to remove) floats one
  // cell clear of anything else and never needed it.
  //
  // Carving used to be here too, on the grounds that world_struck_corner
  // anchors on the terrain's own topmost voxel rather than a slot above it,
  // so the ghost sat inside geometry that was still being drawn. It is not
  // any more: set_virtual_carve_preview injects a real AIR entry, so the
  // voxel is gone from the mesh while the cursor is on it and the ghost has
  // the space to itself. Keeping the bias meant a ghost down inside a
  // carved hole beat the depth test against the ground around it, and its
  // far edges drew as lines running out across the surface.
  const coplanar_with_terrain = !use_object_style;

  const decor_group = decor_bundle && decor_pick ? DECOR_BUNDLES.slice(0, DECOR_BUNDLES.indexOf(decor_bundle)).reduce((n, b) => n + b.variants.length, 0) + decor_pick.variant : -1;
  const ghost_x = hovered ? ivec.to_real(ivec.X(hovered.i)) + fvec.to_real(fvec.X(hovered.f)) : 0;
  const ghost_y = hovered ? ivec.to_real(ivec.Y(hovered.i)) + fvec.to_real(fvec.Y(hovered.f)) : 0;
  const ghost_z = hovered ? ivec.to_real(ivec.Z(hovered.i)) + fvec.to_real(fvec.Z(hovered.f)) : 0;
  const ghost_value = color_id && texture_pack ? placeable_cube_num_id(color_id) : 0;
  const ghost_rgb = parseInt((swatch ?? fallback).slice(1), 16);
  const show_ghost = hovered !== null && !blocked;
  useEffect(() => {
    const p = CORE_PXL;
    if (!p) return;
    if (!show_ghost) {
      p.pxl_ghost_clear();
    } else if (is_decor_ghost && decor_pick && decor_group >= 0) {
      p.pxl_ghost_decor(ghost_x + preview_size / 2, ghost_y + preview_size / 2, ghost_z, decor_group, decor_pick.rotation);
    } else {
      p.pxl_ghost_box(ghost_x, ghost_y, ghost_z, current_scale, ghost_value, ghost_rgb, coplanar_with_terrain ? 1 : 0);
    }
    request_world_tick();
  }, [show_ghost, is_decor_ghost, decor_group, decor_pick?.rotation, ghost_x, ghost_y, ghost_z, preview_size, current_scale, ghost_value, ghost_rgb, coplanar_with_terrain]);
  useEffect(() => () => CORE_PXL?.pxl_ghost_clear(), []);
  return null;
}
