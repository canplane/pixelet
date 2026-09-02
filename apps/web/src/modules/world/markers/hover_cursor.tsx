// @pxl/web/modules/world/markers/hover_cursor.tsx

import { useEffect, useMemo } from "react";
import * as THREE from "three";

import type { PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { center3n, fill3 } from "@pxl/shared/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/shared/bake/constants.ts";

import { useCameraStore } from "@/modules/camera/store.ts";
import { useUiStore } from "@/app/store/ui_store.ts";
import { useMapStore } from "@/modules/map/store.ts";
import { get_local_world_origin } from "../frame/coord.ts";
import { find_overlapping_object } from "../objects/collision.ts";
import { set_virtual_hide, usePlacedObject } from "../objects/store.ts";
import type { ObjectColorId } from "@pxl/shared/spec/cell/objects.ts";
import { get_object_texture } from "../objects/tex.ts";
import { AnchorNode } from "../anchor/node.tsx";
import { marker_colors } from "./style.ts";
import { light_cfg } from "@/modules/world/environment/config.ts";

// Flat, no border: a bordered box only reads cleanly on a primitive with
// per-face UVs mapped 0-1 (this one's own previous implementation) — an
// arbitrary decor mesh's own UV layout has nothing to do with its
// silhouette, so the same trick breaks on anything that isn't a box. A
// screen-space silhouette outline would generalize instead, but that's a
// real post-processing pass, not a per-mesh shader — tracked separately,
// not worth it just for this.
const GHOST_OPACITY = 0.5;

const HIGHLIGHT_BOX = new THREE.BoxGeometry(1, 1, 1);

const TERRAIN_POS = center3n([0, 0, -U_PER_CELL], U_PER_CELL);

export function HoverCursor() {
  const hovered: PVF3 | null = useCameraStore((state) => state.hovered_coord);
  const hovered_object_id = useCameraStore((state) => state.hovered_object_id);
  const activeTool = useUiStore((state) => state.activeTool);
  const object_color_id = useUiStore((state) => state.object_color_id);
  const object_size = useUiStore((state) => state.object_size);
  const map_center = useMapStore((state) => state.map_center);

  const hovered_object = usePlacedObject(hovered_object_id);

  const use_object_style = activeTool === "object" || activeTool === "eraser" || hovered_object !== null;

  const preview_size = (activeTool === "eraser" || activeTool === "pointer") && hovered_object ? hovered_object.size : object_size;

  const object_pos = useMemo(() => center3n(fill3(0), preview_size), [preview_size]);

  const blocked = useMemo(() => {
    if (activeTool === "object") {
      if (!hovered) return false;
      const map_origin = get_local_world_origin(map_center);
      return find_overlapping_object(hovered, object_size, map_origin) !== null;
    }
    if (activeTool === "eraser") return hovered_object === null;
    return false;
  }, [activeTool, hovered, map_center, object_size, hovered_object]);

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

  // Pointer (select/move) always reads as the generic marker color, whether
  // it's hovering bare terrain or an existing object — the two need to look
  // identical (same cyan/sky-blue ghost) since neither is "here's the real
  // thing you're about to add or remove", just "here's what you could grab".
  // Object placement and eraser instead ghost at the real thing's own true
  // color_id — the placement candidate's for object mode, the existing
  // object's own for eraser — since both of those really are "here's the
  // real thing". Bare terrain with no real color_id to borrow (pointer, or
  // object mode with no candidate color context) has none.
  const color_id: ObjectColorId | null = activeTool === "pointer" ? null : hovered_object ? hovered_object.color_id : activeTool === "object" ? object_color_id : null;

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
  const color: string = map ? "#ffffff" : marker_colors.ACTION;

  const current_scale = use_object_style ? preview_size : U_PER_CELL;

  if (!hovered || blocked) return null;
  return (
    <AnchorNode i={hovered.i} f={hovered.f}>
      <group position={use_object_style ? object_pos : TERRAIN_POS}>
        <mesh
          geometry={HIGHLIGHT_BOX}
          scale={current_scale}
          // 9999 pairs with the terrain case's strong polygon offset below;
          // object mode uses the default order so real depth occlusion works.
          renderOrder={use_object_style ? 0 : 9999}
        >
          <meshStandardMaterial
            color={color}
            map={map}
            // Lit, not meshBasicMaterial's flat/unlit fill: the goal is "a
            // real render of the thing, just half-transparent", not a flat
            // color swatch shaped like it — an unlit ghost reads as a UI
            // highlight overlay, a lit one reads as the thing itself. Plain
            // MeshStandardMaterial (roughness/metalness matched to
            // chunk_stage.tsx's own create_material()), not that file's
            // actual material: that one's onBeforeCompile hard-requires
            // per-instance attributes (INSTANCE_RGB_ATTR/LAYER_TAPS/VISIBLE)
            // and instanceMatrix that only exist on a real ChunkInstanceMesh
            // — reusing it here would mean wrapping this single ghost box in
            // a 1-count InstancedMesh just to satisfy that contract, real
            // coupling to chunk_stage.tsx's own instancing internals for a
            // rim-light/terrain-layer-texturing payoff this preview doesn't
            // need — the actual texture pattern (this material's own `map`
            // above) already comes from the same source PNG a real object
            // renders with, just loaded as a plain Texture instead of
            // through that shader's DataArrayTexture.
            //
            // emissive+emissiveMap+emissiveIntensity mirrors
            // environment/config.ts's shared light_cfg.FLOOR — the exact
            // same minimum-brightness floor every other real-lit surface in
            // the scene gets (terrain and objects alike, via
            // chunk_stage.tsx's own shader injection: `totalEmissiveRadiance
            // = base_rgb * FLOOR`, base_rgb there being the sampled texture
            // itself, not a flat color). emissiveMap (not just emissive)
            // matters here for the same texture-vs-flat-color reason as map
            // above: without it the floor would glow as a flat tinted
            // rectangle in the dark instead of dimly showing the same
            // texture pattern the lit portion shows.
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
            // Object mode needs only a small polygon-offset nudge (it's not
            // coplanar with a distant surface like the terrain case is) —
            // reusing the terrain case's strong bias here would also push
            // the cursor in front of a placed cube that should legitimately
            // occlude it.
            polygonOffsetFactor={use_object_style ? -1 : -2}
            polygonOffsetUnits={use_object_style ? -1 : -8}
          />
        </mesh>
      </group>
    </AnchorNode>
  );
}
