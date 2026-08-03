// @pxl/web/modules/world/markers/cell-highlighter.tsx

import { useMemo } from "react";
import * as THREE from "three";

import type { PVF3 } from "@pixelet/shared/spec/space/pvf3/mod.ts";
import { center3n } from "@pixelet/shared/spec/space/v3.ts";

import { U_PER_CELL } from "@pixelet/shared/bake/constants.ts";

import { useCameraStore } from "@/modules/camera/store.ts";
import { AnchorNode } from "../spatial/anchor/node.tsx";

// Highlighter style
const highlight_style = {
  COLOR: "#ff9f05",
  FILL_OPACITY: .6,
  BORDER_WIDTH: .05,
};

const HIGHLIGHT_BOX = new THREE.BoxGeometry(1, 1, 1);

const POS = center3n([0, 0, -U_PER_CELL], U_PER_CELL);

export function CellHighlighter() {
  const hovered: PVF3 | null = useCameraStore((state) => state.hovered_coord);

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

  if (!hovered) return null;
  return (
    <AnchorNode i={hovered.i} f={hovered.f}>
      <group position={POS}>
        <mesh
          geometry={HIGHLIGHT_BOX}
          material={mat}
          scale={U_PER_CELL}
          renderOrder={9999}
        />
      </group>
    </AnchorNode>
  );
}
