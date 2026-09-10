// @pxl/web/modules/world/markers/hover_cursor.tsx

import { useEffect, useMemo } from "react";
import * as THREE from "three";

import type { f64 } from "@xpute/core/kit/word.ts";
import type { PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { center3n, fill3 } from "@pxl/shared/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/shared/bake/constants.ts";

import { useCameraStore } from "@/modules/camera/store.ts";
import { useUiStore } from "@/app/store/ui_store.ts";
import { useMapStore } from "@/modules/map/store.ts";
import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { get_local_world_origin } from "../frame/coord.ts";
import { is_placement_blocked } from "../objects/collision.ts";
import { DECOR_SIZE } from "../objects/size.ts";
import { clear_virtual_carve_preview, set_virtual_carve_preview, set_virtual_hide, usePlacedObject } from "../objects/store.ts";
import type { PlaceableCubeId } from "@pxl/shared/spec/cell/mod.ts";
import { type CubeFace, get_object_texture } from "../objects/tex.ts";
import { user_decor_pick } from "../decor/decor_layer.ts";
import { decor_bundle_for } from "../decor/decor_registry.ts";
import { compose_instance_matrix } from "../decor/instanced_model.ts";
import { decor_assets_for, useDecorAssets } from "../decor/asset_store.ts";
import { AnchorNode } from "../anchor/node.tsx";
import { marker_colors } from "./style.ts";
import { shown_ground } from "@pxl/shared/bake/ground.ts";
import { placeable_cube_hex, terrain_style_cube_for } from "@pxl/shared/spec/cell/mod.ts";
import { get_ground_type } from "../terrain/elevation.ts";
import { light_cfg } from "@/modules/world/environment/config.ts";

// Flat, no border: a bordered box only reads cleanly on a primitive with
// per-face UVs mapped 0-1 (this one's own previous implementation) — an
// arbitrary decor mesh's own UV layout has nothing to do with its
// silhouette, so the same trick breaks on anything that isn't a box. Decor
// specifically no longer needs this fallback (see decor_geometry below —
// it gets its own real silhouette instead), but the box stays as the
// treatment for everything else (cube placement, plain terrain/pointer)
// this file's own doc comment always described. A screen-space silhouette
// outline would generalize the border further, but that's a real
// post-processing pass, not a per-mesh shader — tracked separately, not
// worth it just for this.
//
// Half-transparent, and it has to actually reach the eye as half: a closed
// box rendered DoubleSide composites its far faces under its near ones, so
// the same 0.5 arrives as ~0.75 and the carved hole behind it stops
// reading. The box ghost is therefore FrontSide — it is closed, so its back
// faces carry nothing the front ones don't. A decor silhouette is an
// arbitrary mesh with no such guarantee and keeps DoubleSide.
const GHOST_OPACITY = 0.5;

/** A unit box whose UVs span the box's own world extent rather than 0..1.
 *
 * The real chunk shader tiles a texture once per world unit: chunk_mesher.ts
 * writes `a_box_scale` in u, and chunk_material.ts's `v_box_uv` is the local
 * position times that. BoxGeometry's own UVs span the whole box however
 * large it is, so a ghost agrees with the render at one unit and stretches
 * the image at every other size. Cached because sizes come from a short
 * fixed tier list. */
const highlight_boxes = new Map<f64, THREE.BoxGeometry>();

function highlight_box(extent_u: f64): THREE.BoxGeometry {
  const cached = highlight_boxes.get(extent_u);
  if (cached) return cached;
  const geometry = new THREE.BoxGeometry(1, 1, 1);
  const uv = geometry.attributes.uv as THREE.BufferAttribute;
  for (let i = 0; i < uv.count; i++) uv.setXY(i, uv.getX(i) * extent_u, uv.getY(i) * extent_u);
  highlight_boxes.set(extent_u, geometry);
  return geometry;
}

// Stable empty reference for the renders where no decor bundle applies, so
// nothing downstream sees a fresh `[]` identity every frame.
const EMPTY_ASSETS: readonly null[] = [];

// Scratch, reused every render — compose_instance_matrix's own real bake
// output, decomposed into just the rotation/scale a ghost mesh needs
// (position is always zero here: the ghost's own position comes from the
// group wrapper below, exactly like the real per-instance bake's own
// translation would, just supplied by AnchorNode/decor_ghost_pos instead of
// baked into this matrix).
const $decor_matrix = new THREE.Matrix4();
const $decor_translation = new THREE.Vector3();
const $decor_quaternion = new THREE.Quaternion();
const $decor_scale = new THREE.Vector3();

// BoxGeometry's six groups in three.js's own order: +x, -x, +y, -y, +z, -z.
// Z is up here, so the first four are the lateral walls and the last two are
// the caps. chunk_material.ts draws the same line at `abs(normal.z) < 0.5`,
// which is why the bottom cap wears the top image rather than the side one.
const BOX_FACE_TEXTURES: readonly CubeFace[] = ["side", "side", "side", "side", "top", "top"];
const TOP_FACE = 4;

type GhostMaterialProps = {
  attach?: string;
  map: THREE.Texture | null;
  color: string;
  side: THREE.Side;
  polygonOffsetFactor: number;
  polygonOffsetUnits: number;
};

/**
 * The one ghost-preview material shape both hover ghosts below use (the
 * decor silhouette and the generic cube/terrain box) — lit
 * (MeshStandardMaterial), not meshBasicMaterial's flat/unlit fill: the goal
 * is "a real render of the thing, just half-transparent," not a flat color
 * swatch shaped like it — an unlit ghost reads as a UI highlight overlay, a
 * lit one reads as the thing itself. Plain MeshStandardMaterial (roughness/
 * metalness matched to chunk_stage.tsx's own create_material()), not that
 * file's actual material or instanced_model.ts's own decor material: both
 * hard-require baked per-vertex attributes or a live shader injection this
 * single translucent preview mesh doesn't have — real coupling to a baking/
 * shader-injection pipeline for a rim-light/terrain-layer-texturing payoff
 * neither ghost needs.
 *
 * emissive+emissiveMap+emissiveIntensity mirrors environment/config.ts's
 * shared light_cfg.FLOOR — the same minimum-brightness floor every other
 * real-lit surface in the scene gets, so a ghost never reads darker than a
 * real instance would in the same spot. emissiveMap (not just emissive)
 * matters for the same texture-vs-flat-color reason `map` does: without it
 * the floor would glow as a flat tinted rectangle in the dark instead of
 * dimly showing the same texture pattern the lit portion shows. `color`
 * doubles as `emissive` directly — both ghosts want the exact same hue lit
 * and floor-lit, never two independently-tuned colors.
 */
function GhostMaterial({ attach, map, color, side, polygonOffsetFactor, polygonOffsetUnits }: GhostMaterialProps) {
  return (
    <meshStandardMaterial
      attach={attach}
      map={map}
      color={color}
      emissive={color}
      emissiveMap={map}
      emissiveIntensity={light_cfg.FLOOR}
      roughness={1}
      metalness={0}
      transparent
      opacity={GHOST_OPACITY}
      side={side}
      depthTest
      polygonOffset
      polygonOffsetFactor={polygonOffsetFactor}
      polygonOffsetUnits={polygonOffsetUnits}
    />
  );
}

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
  const low_spec = useSettingsStore((state) => state.low_spec);

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
  const ghost_pos = useMemo(() => center3n(fill3(0), current_scale), [current_scale]);
  // current_scale is already in u, and unrounded: a sub-cell ghost samples a
  // matching fraction of the texture, which is what the real render does
  // with one too.
  const ghost_geometry = useMemo(() => highlight_box(current_scale), [current_scale]);

  // Same "object size vs decor's own fixed DECOR_SIZE" dispatch
  // camera/input.tsx's own handle_tap needs to actually commit a placement —
  // objects/collision.ts's own is_placement_blocked is the one shared place
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
  // eraser hovering a decor entry) never reach this — decor_geometry below
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
  const decor_geometries = decor_assets?.geometries ?? EMPTY_ASSETS;
  const decor_materials = decor_assets?.materials ?? EMPTY_ASSETS;

  // The exact `i` a real placement/hover resolves to — user_decor_pick is
  // pure and keyed only on this, so calling it here predicts (for a
  // placement candidate) or recovers (for an already-placed entry) the
  // real variant/heading a real render would use, not an arbitrary stand-in.
  const decor_pick_i = activeTool === "decor" ? hovered?.i ?? null : hovered_object?.kind === "decor" ? hovered_object.i : null;
  const decor_pick = decor_bundle && decor_pick_i !== null ? user_decor_pick(decor_pick_i, decor_bundle.variants.length) : null;
  const decor_geometry = decor_pick ? decor_geometries[decor_pick.variant] ?? null : null;
  const decor_map = decor_pick ? decor_materials[decor_pick.variant]?.map ?? null : null;
  // Both must be ready — an untextured white silhouette (geometry loaded,
  // texture not yet) would read as a real rendering glitch, not a loading
  // state, so this falls back to the flat box ghost until both resolve.
  const is_decor_ghost = decor_geometry !== null && decor_map !== null;

  if (is_decor_ghost && decor_pick) {
    // Position is intentionally zero here — real decor's own position comes
    // from the group wrapper below (AnchorNode sits at hovered.i/f and the
    // wrapper offsets to the cell's horizontal center), so only rotation/
    // scale from this matrix are used.
    // scale is always 1: real user-placed decor never gets scatter.ts's own
    // jitter (see objects/store.ts's "decor" channel doc comment) — only
    // procedural placements do, and those are never hoverable/selectable.
    compose_instance_matrix($decor_matrix, { position: [0, 0, 0], rotation_z: decor_pick.rotation, scale: 1, id: null });
    $decor_matrix.decompose($decor_translation, $decor_quaternion, $decor_scale);
  }

  // The real thing's own actual painted texture (objects/tex.ts's own
  // cached, pixel-art-filtered load — the exact file a real placed cube of
  // this color renders with), not just its flat hex swatch: a flat color
  // was reading as an "unlit UI highlight" shape with the real surface
  // detail gone, even after the material itself became lit (see this file's
  // own emissive/roughness comment below) — texture is what a real render
  // actually looks like, lighting alone wasn't enough. Loaded directly from
  // the source PNG via a plain THREE.Texture, not chunk_stage.tsx's own
  // DataArrayTexture + per-instance-layer-index shader: that path needs the
  // same per-instance attributes and instanceMatrix this file's own
  // material comment already ruled out reusing, for the exact same reason.
  // `color` becomes white whenever a real texture is used — MeshStandardMaterial
  // multiplies map by color, and the texture already carries the real hue,
  // so leaving color at the hex value would double-tint it.
  //
  // Textures follow the real render: with the texture pack off a real cube
  // is a flat colour, so a ghost showing a texture there would not be a
  // half-transparent copy of anything on screen.
  //
  // Per face, because a cube's walls are not its lid: bare ground is grass
  // on top and dirt down the side, and a ghost wearing the lid on all six
  // faces is not a copy of the thing it stands for. Only the land channel
  // declares side images today, so everything else resolves to the same
  // texture on every face and pays nothing for asking.
  const textures = useMemo(
    () => BOX_FACE_TEXTURES.map((face) => (color_id && !low_spec ? get_object_texture(color_id, face) : null)),
    [color_id, low_spec],
  );
  const map = textures[TOP_FACE];

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
  const color: string = map ? "#ffffff" : swatch ?? fallback;

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

  if (!hovered || blocked) return null;

  // Decor's own real silhouette+texture — matching how a real
  // DecorVariantMesh actually renders (see GhostMaterial's own doc comment
  // for the lit/emissive-floor reasoning shared with the generic ghost
  // below). No rim-light injection here though — that's GhostMaterial's own
  // choice too, for the same reason. Horizontally centered in the hovered
  // cell (+preview_size/2 on X/Y, matching decor_layer.ts's own
  // collect_user_decor_instances — every decor lands at its footprint's
  // middle, not a corner), Z left at the anchor (the model's own origin sits
  // at its base, which rests on the on-top slot world_adjacent_corner
  // already resolved).
  if (is_decor_ghost && decor_geometry && decor_map) {
    return (
      <AnchorNode i={hovered.i} f={hovered.f}>
        <group position={[preview_size / 2, preview_size / 2, 0]}>
          <mesh geometry={decor_geometry} quaternion={$decor_quaternion} scale={$decor_scale} renderOrder={0}>
            <GhostMaterial map={decor_map} color="#ffffff" side={THREE.DoubleSide} polygonOffsetFactor={-1} polygonOffsetUnits={-1} />
          </mesh>
        </group>
      </AnchorNode>
    );
  }

  return (
    <AnchorNode i={hovered.i} f={hovered.f}>
      <group position={ghost_pos}>
        <mesh
          geometry={ghost_geometry}
          scale={current_scale}
          // 9999 pairs with the coplanar case's strong polygon offset below;
          // a floating (non-coplanar) ghost uses the default order so real
          // depth occlusion against everything else still works.
          renderOrder={coplanar_with_terrain ? 9999 : 0}
        >
          {
            // The actual texture pattern (`map`) already comes from the same
            // source PNG a real object renders with (objects/tex.ts), just
            // loaded as a plain Texture instead of through chunk_stage.tsx's
            // own DataArrayTexture — see GhostMaterial's own doc comment for
            // why that file's real material isn't reused here.
            //
            // A floating ghost needs only a small polygon-offset nudge (it's
            // not coincident with a distant surface the way the coplanar
            // case is) — reusing the coplanar case's strong bias here would
            // also push the cursor in front of a placed cube that should
            // legitimately occlude it.
            //
            // One material per box group, so the walls can differ from the
            // lid. They share a THREE.Texture whenever the kind declares no
            // side of its own, which is every kind but land.
          }
          {textures.map((face_map, i) => (
            <GhostMaterial
              key={i}
              attach={`material-${i}`}
              map={face_map}
              color={color}
              side={THREE.FrontSide}
              polygonOffsetFactor={coplanar_with_terrain ? -2 : -1}
              polygonOffsetUnits={coplanar_with_terrain ? -8 : -1}
            />
          ))}
        </mesh>
      </group>
    </AnchorNode>
  );
}
