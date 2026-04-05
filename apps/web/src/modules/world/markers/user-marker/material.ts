// @pxl/web/modules/world/markers/user-marker/material.ts

import * as THREE from "three/webgpu";
import { attribute, color, float, fwidth, mix, oneMinus, smoothstep, uv } from "three/tsl";

const zone_style = {
  COLOR: "#4fd1c5",
  FILL_OPACITY: .4,
  BORDER_WIDTH: .085,
} as const;

type Vec4Node = any;
type FloatNode = ReturnType<typeof float>;

function as_vec4_node(x: unknown): Vec4Node {
  return x as Vec4Node;
}

function as_float_node(x: unknown): FloatNode {
  return x as FloatNode;
}

function aastep(thres: FloatNode, value: FloatNode): FloatNode {
  const afwidth = fwidth(value).mul(.5);
  return smoothstep(thres.sub(afwidth), thres.add(afwidth), value) as FloatNode;
}

export function create_zone_material(): THREE.MeshBasicNodeMaterial {
  const material = new THREE.MeshBasicNodeMaterial();

  const v_uv = uv();
  const v_borders = as_vec4_node(attribute("a_borders", "vec4"));

  const border_width = float(zone_style.BORDER_WIDTH);
  const fill_opacity = float(zone_style.FILL_OPACITY);

  const uv_x = as_float_node(v_uv.x);
  const uv_y = as_float_node(v_uv.y);

  const border_top_mask = as_float_node(v_borders.x);
  const border_bottom_mask = as_float_node(v_borders.y);
  const border_right_mask = as_float_node(v_borders.z);
  const border_left_mask = as_float_node(v_borders.w);

  const border_top = aastep(oneMinus(border_width) as FloatNode, uv_y).mul(border_top_mask);
  const border_bottom = oneMinus(aastep(border_width, uv_y)).mul(border_bottom_mask);
  const border_right = aastep(oneMinus(border_width) as FloatNode, uv_x).mul(border_right_mask);
  const border_left = oneMinus(aastep(border_width, uv_x)).mul(border_left_mask);

  const border_strength = border_top
    .max(border_bottom)
    .max(border_left)
    .max(border_right);
  const alpha = mix(fill_opacity, float(1), border_strength);

  material.colorNode = color(zone_style.COLOR);
  material.opacityNode = alpha;

  material.transparent = true;
  material.depthWrite = false;
  material.depthTest = true;
  material.side = THREE.DoubleSide;
  material.polygonOffset = true;
  material.polygonOffsetFactor = -1.0;
  material.polygonOffsetUnits = -4.0;

  return material;
}
