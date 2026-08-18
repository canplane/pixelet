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

const highlight_style = {
  COLOR: marker_colors.ACTION,
  FILL_OPACITY: .4,
  BORDER_WIDTH: 0.04,
};

const HIGHLIGHT_BOX = new THREE.BoxGeometry(1, 1, 1);

const TERRAIN_POS = center3n([0, 0, -U_PER_CELL], U_PER_CELL);

export function HoverCursor() {
  const hovered: PVF3 | null = useCameraStore((state) => state.hovered_coord);
  const hovered_object_id = useCameraStore((state) => state.hovered_object_id);
  const active_tool = useUiStore((state) => state.active_tool);
  const object_color_id = useUiStore((state) => state.object_color_id);
  const object_size = useUiStore((state) => state.object_size);
  const map_center = useMapStore((state) => state.map_center);

  const hovered_object = usePlacedObject(hovered_object_id);

  const use_object_style = active_tool === "object" || active_tool === "eraser" || hovered_object !== null;

  const preview_size = (active_tool === "eraser" || active_tool === "pointer") && hovered_object ? hovered_object.size : object_size;

  const object_pos = useMemo(() => center3n(fill3(0), preview_size), [preview_size]);

  const blocked = useMemo(() => {
    if (active_tool === "object") {
      if (!hovered) return false;
      const map_origin = get_local_world_origin(map_center);
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

  let color: string = highlight_style.COLOR;
  const opacity: number = highlight_style.FILL_OPACITY;
  if (active_tool === "object") color = object_color_hex(object_color_id);
  else if (active_tool === "eraser") color = marker_colors.DESTRUCTIVE;

  const current_scale = use_object_style ? preview_size : U_PER_CELL;

  useLayoutEffect(() => {
    mat.uniforms.u_color.value.set(color);
    mat.uniforms.u_opacity.value = opacity;
    // Object mode needs only a small polygon-offset nudge (it's not
    // coplanar with a distant surface like the terrain case is) — reusing
    // the terrain case's strong bias here would also push the cursor in
    // front of a placed cube that should legitimately occlude it.
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
          // 9999 pairs with the terrain case's strong polygon offset above;
          // object mode uses the default order so real depth occlusion works.
          renderOrder={use_object_style ? 0 : 9999}
        />
      </group>
    </AnchorNode>
  );
}
