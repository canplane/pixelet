// @pxl/web/modules/world/markers/hover_cursor.tsx

import { useLayoutEffect, useMemo } from "react";
import * as THREE from "three";

import type { PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { center3n, fill3 } from "@pxl/shared/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/shared/bake/constants.ts";

import { useCameraStore } from "@/modules/camera/store.ts";
import { useUiStore } from "@/app/store/ui_store.ts";
import { useMapStore } from "@/modules/map/store.ts";
import { get_local_world_origin } from "../frame/coord.ts";
import { find_overlapping_object } from "../objects/collision.ts";
import { usePlacedObject } from "../objects/store.ts";
import { object_color_hex } from "@pxl/shared/spec/cell/objects.ts";
import { AnchorNode } from "../anchor/node.tsx";
import { marker_colors } from "./style.ts";

// terrain-cell highlight style (pointer tool over bare ground) — embeds
// into the hovered cell. Same color and opacity are reused (object-shaped)
// when the pointer tool is aimed at an object instead — see
// use_object_style below. Color itself is location_puck.tsx's own zone-ring
// teal (see that constant's own comment) rather than a separate hex here —
// both mark "you/your own actions", so sharing one color language reads as
// intentional, and it stays distinct from the eraser's destructive red and
// whatever palette color the object tool is currently placing (see
// marker_colors.DESTRUCTIVE below).
const highlight_style = {
  COLOR: marker_colors.ACTION,
  FILL_OPACITY: .4,
  BORDER_WIDTH: 0.04,
};

// placement/erase preview style (object/eraser tools) — sits on top of the
// terrain hit, see object_pos below. Border width and fill opacity
// unchanged from the terrain highlight — only color/position differ by
// mode. Eraser gets its own destructive-red instead of reusing the terrain
// highlight's own color (no palette-color relation either way) — the two
// mean different things (hover/select vs. about to remove this) and
// shouldn't share one color.

const HIGHLIGHT_BOX = new THREE.BoxGeometry(1, 1, 1);

// terrain-cell highlight: the cell one level below the hit surface, centered.
const TERRAIN_POS = center3n([0, 0, -U_PER_CELL], U_PER_CELL);

export function HoverCursor() {
  const hovered: PVF3 | null = useCameraStore((state) => state.hovered_coord);
  const hovered_object_id = useCameraStore((state) => state.hovered_object_id);
  const active_tool = useUiStore((state) => state.active_tool);
  const object_color_id = useUiStore((state) => state.object_color_id);
  const object_size = useUiStore((state) => state.object_size);
  const map_center = useMapStore((state) => state.map_center);

  // the actual object under the cursor, if any — eraser and pointer both
  // target the struck object directly (camera/input.tsx's update_hover),
  // unlike "object" tool which computes an adjacent empty slot instead and
  // always leaves hovered_object_id null. Its own size (not the palette's
  // object_size) is what the outline should match, so it always traces
  // exactly what would be removed/selected.
  const hovered_object = usePlacedObject(hovered_object_id);

  // pointer tool keeps the classic embedded terrain-cell highlight when
  // hovering bare ground, but switches to the same "sits on top, traces the
  // real footprint" treatment as eraser when it's pointed at an object —
  // a flat grid-cell indicator doesn't generalize to "here's the object
  // you're about to jump to," so this only flips for that specific case
  // rather than changing the terrain-hover look everywhere.
  const use_object_style = active_tool === "object" || active_tool === "eraser" || hovered_object !== null;

  const preview_size = (active_tool === "eraser" || active_tool === "pointer") && hovered_object ? hovered_object.size : object_size;

  // object placement preview: the cell at the hit surface (terrain, or —
  // now that camera/ray.ts's ray_pick does real object raycasting — flush
  // against whatever face was clicked), centered — matches chunk_mesher.ts's
  // own box-center convention (min + size/2) for the real placed cube.
  const object_pos = useMemo(() => center3n(fill3(0), preview_size), [preview_size]);

  const blocked = useMemo(() => {
    if (active_tool === "object") {
      if (!hovered) return false;
      const map_origin = get_local_world_origin(map_center);
      // hovered is already the adjacent-face corner when an object was hit
      // (see camera/input.tsx's update_hover) — this only trips when THAT
      // candidate slot is itself already occupied by another object.
      return find_overlapping_object(hovered, object_size, map_origin) !== null;
    }
    if (active_tool === "eraser") return hovered_object === null;
    return false;
  }, [active_tool, hovered, map_center, object_size, hovered_object]);

  const mat: THREE.ShaderMaterial = useMemo(
    () =>
      new THREE.ShaderMaterial({
        extensions: { derivatives: true } as any,
        uniforms: {
          u_color: { value: new THREE.Color(highlight_style.COLOR) },
          u_opacity: { value: highlight_style.FILL_OPACITY },
          u_border_width: { value: highlight_style.BORDER_WIDTH },
        },
        vertexShader: `varying vec2 v_uv;
void main() {
    v_uv = uv;
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}`,
        fragmentShader: `uniform vec3 u_color;
uniform float u_opacity;
uniform float u_border_width;
varying vec2 v_uv;

float aastep(float thres, float value) {
    float afwidth = fwidth(value) * 0.5;
    return smoothstep(thres - afwidth, thres + afwidth, value);
}

void main() {
    float bx = aastep(u_border_width, v_uv.x) * aastep(v_uv.x, 1.0 - u_border_width);
    float by = aastep(u_border_width, v_uv.y) * aastep(v_uv.y, 1.0 - u_border_width);

    float is_center = bx * by;
    float is_border = 1.0 - is_center;

    float alpha = mix(u_opacity, 1.0, is_border);
    gl_FragColor = vec4(u_color, alpha);
}`,
        transparent: true,
        side: THREE.DoubleSide,
        depthTest: true,
        polygonOffset: true,
        polygonOffsetFactor: -2.0,
        polygonOffsetUnits: -8.0,
      }),
    [],
  );

  // blocked only ever applies to the object/eraser tools (see the blocked
  // useMemo above), and suppresses the cursor entirely (see the early
  // return below) — nothing to compute a blocked color/opacity for.
  // pointer's use_object_style case (aimed at an object) falls through to
  // the plain highlight_style.COLOR default, same zone-teal language as the
  // terrain-cell highlight, just object-shaped. eraser has no relation to
  // object color either, so it never reads object_color_hex — it gets its
  // own shared destructive marker color instead.
  let color: string = highlight_style.COLOR;
  const opacity: number = highlight_style.FILL_OPACITY;
  if (active_tool === "object") color = object_color_hex(object_color_id);
  else if (active_tool === "eraser") color = marker_colors.DESTRUCTIVE;

  const current_scale = use_object_style ? preview_size : U_PER_CELL;

  useLayoutEffect(() => {
    mat.uniforms.u_color.value.set(color);
    mat.uniforms.u_opacity.value = opacity;
    // The terrain-cell case is coplanar with the (potentially large,
    // distant) terrain surface it's embedded in, so it needs a strong
    // negative bias to reliably win that z-fight from any distance/angle.
    // That same strong bias, reused for the object-shaped case, was large
    // enough to also fake past legitimate occlusion — a placed cube
    // actually in front of the cursor should hide it, but the bias pushed
    // the cursor's depth value in front regardless, breaking that. Object
    // mode only ever needs a small nudge (exact-coincidence z-fight against
    // an eraser target, or a touching seam against an adjacent placement
    // preview), so a much smaller bias there leaves real occlusion intact.
    mat.polygonOffsetFactor = use_object_style ? -1 : -2;
    mat.polygonOffsetUnits = use_object_style ? -1 : -8;
  }, [mat, color, opacity, use_object_style]);

  if (!hovered || blocked) return null;
  return (
    <AnchorNode i={hovered.i} f={hovered.f}>
      <group position={use_object_style ? object_pos : TERRAIN_POS}>
        <mesh
          geometry={HIGHLIGHT_BOX}
          material={mat}
          scale={current_scale}
          // terrain-cell mode still forces draw-order priority (9999) to
          // pair with its own strong polygon offset above; object mode
          // uses the default order so the transparent-queue's normal
          // back-to-front + depth-test sort can actually occlude it.
          renderOrder={use_object_style ? 0 : 9999}
        />
      </group>
    </AnchorNode>
  );
}
