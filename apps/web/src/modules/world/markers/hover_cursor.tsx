// @pxl/web/modules/world/markers/hover_cursor.tsx

import { useEffect, useMemo } from "react";
import * as THREE from "three";

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
import { get_object_texture } from "../objects/tex.ts";
import { user_decor_pick } from "../decor/decor_layer.tsx";
import { decor_bundle_for } from "../decor/decor_registry.ts";
import { compose_instance_matrix, type DecorVariantAsset, useInstancedModelVariants } from "../decor/instanced_model.ts";
import { AnchorNode } from "../anchor/node.tsx";
import { marker_colors } from "./style.ts";
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
const GHOST_OPACITY = 0.5;

const HIGHLIGHT_BOX = new THREE.BoxGeometry(1, 1, 1);

// Stable empty reference — passed to useInstancedModelVariants whenever no
// decor bundle is relevant this render, so its own internal variants_key
// stays a constant "" instead of churning every render a fresh `[]`
// literal would cause.
const EMPTY_VARIANTS: readonly DecorVariantAsset[] = [];

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

type GhostMaterialProps = {
  map: THREE.Texture | null;
  color: string;
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
function GhostMaterial({ map, color, polygonOffsetFactor, polygonOffsetUnits }: GhostMaterialProps) {
  return (
    <meshStandardMaterial
      map={map}
      color={color}
      emissive={color}
      emissiveMap={map}
      emissiveIntensity={light_cfg.FLOOR}
      roughness={1}
      metalness={0}
      transparent
      opacity={GHOST_OPACITY}
      side={THREE.DoubleSide}
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
  const color_id: PlaceableCubeId | null = activeTool === "pointer" || activeTool === "decor"
    ? null
    : hovered_object?.kind === "solid"
    ? hovered_object.color_id
    : activeTool === "object"
    ? object_color_id
    : null;

  // Which decor kind's own real silhouette this ghost should show, if any —
  // the kind about to be placed (decor tool active) or the kind already
  // sitting where an eraser/pointer hover landed. `useInstancedModelVariants`
  // is called unconditionally, once, with whichever bundle's variants (or
  // the stable EMPTY_VARIANTS) currently applies — its own internal loading
  // effects are keyed by a content hash of the variant list, so switching
  // between decor kinds (or away from decor entirely) just re-triggers
  // exactly the loads that actually changed, same as any other consumer of
  // that hook.
  const decor_id = activeTool === "decor" ? ui_decor_id : hovered_object?.kind === "decor" ? hovered_object.decor_id : null;
  const decor_bundle = decor_id ? decor_bundle_for(decor_id) : null;
  const { geometries: decor_geometries, materials: decor_materials } = useInstancedModelVariants(decor_bundle?.variants ?? EMPTY_VARIANTS);

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
  const map = color_id ? get_object_texture(color_id) : null;
  // Eraser hovering bare terrain (about to dig into it, not remove a real
  // placed thing — see color_id's own doc comment) gets the dark VOID marker
  // instead of pointer's neutral cyan ACTION one, so it reads as "this
  // becomes nothing" rather than "here's what you could grab".
  const color: string = map ? "#ffffff" : digging_terrain ? marker_colors.VOID : marker_colors.ACTION;

  // Whether this ghost sits flush/coincident with real, already-rendered
  // solid geometry rather than floating clear of anything else — decides
  // which of the two polygon-offset/renderOrder biases below actually wins
  // the depth test, independently of current_scale / where the box sits.
  // Pointer/no-candidate-context's own bare-terrain ghost (use_object_style
  // === false) was always this case — a normal "object mode" ghost (a
  // placement candidate, or an existing object eraser is about to remove)
  // floats one cell clear of anything else and never needed it. Eraser
  // digging into bare terrain is the one
  // use_object_style === true case that's ALSO coincident: unlike a real
  // placed object, world_struck_corner's own anchor (see camera/ray.ts) is
  // deliberately the terrain's own real topmost voxel, not a slot above it,
  // so this ghost occupies the exact same space real terrain geometry
  // already renders into. Reusing the small "object mode" bias there left
  // it losing the depth test almost every frame — invisible, not merely
  // faint.
  const coplanar_with_terrain = !use_object_style || digging_terrain;

  if (!hovered || blocked) return null;

  // Decor's own real silhouette+texture — matching how a real
  // DecorVariantMesh actually renders (see GhostMaterial's own doc comment
  // for the lit/emissive-floor reasoning shared with the generic ghost
  // below). No rim-light injection here though — that's GhostMaterial's own
  // choice too, for the same reason. Horizontally centered in the hovered
  // cell (+preview_size/2 on X/Y, matching decor_layer.tsx's own
  // collect_user_decor_instances — every decor lands at its footprint's
  // middle, not a corner), Z left at the anchor (the model's own origin sits
  // at its base, which rests on the on-top slot world_adjacent_corner
  // already resolved).
  if (is_decor_ghost && decor_geometry && decor_map) {
    return (
      <AnchorNode i={hovered.i} f={hovered.f}>
        <group position={[preview_size / 2, preview_size / 2, 0]}>
          <mesh geometry={decor_geometry} quaternion={$decor_quaternion} scale={$decor_scale} renderOrder={0}>
            <GhostMaterial map={decor_map} color="#ffffff" polygonOffsetFactor={-1} polygonOffsetUnits={-1} />
          </mesh>
        </group>
      </AnchorNode>
    );
  }

  return (
    <AnchorNode i={hovered.i} f={hovered.f}>
      <group position={ghost_pos}>
        <mesh
          geometry={HIGHLIGHT_BOX}
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
          }
          <GhostMaterial
            map={map}
            color={color}
            polygonOffsetFactor={coplanar_with_terrain ? -2 : -1}
            polygonOffsetUnits={coplanar_with_terrain ? -8 : -1}
          />
        </mesh>
      </group>
    </AnchorNode>
  );
}
