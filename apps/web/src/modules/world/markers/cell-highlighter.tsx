// @pxl/web/modules/world/markers/cell-highlighter.tsx

import { useEffect, useMemo, useState } from "react";
import * as THREE from "three";
import type { ThreeEvent } from "@react-three/fiber";
import { useThree } from "@react-three/fiber";

import type { f64, u32 } from "@xpute/spec/lib/kit/word.ts";

import { type IVec, ivec, projection, type PVF3, pvf3 } from "@pxl/spec/space/pvf3/mod.ts";
import { center3, floor3, type V3 } from "@pxl/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/bake/terrain/constants.ts";

import { AnchorNode } from "@/modules/world/spatial/anchor/node.tsx";

import { useMapStore } from "@/modules/map/navigation/map-store.ts";
import { get_ele, with_ele } from "../terrain/use-elevation.ts";
import { CameraControlMode } from "../render/config.ts";

// Raymarching config
const STEP: f64 = 2 * U_PER_CELL;
const MAX_STEPS: u32 = 200;
const BIN_ITER: u32 = 6;
const HIT_EPS: f64 = 0.1 * U_PER_CELL;

// Interaction radius in cell units, with a half-diagonal shave so snapped cell boxes stay inside the intended circular range.
const INTERACT_RADIUS_CELL: f64 = 32 / U_PER_CELL - 0.707;
const INTERACT_RADIUS_U: f64 = INTERACT_RADIUS_CELL * U_PER_CELL;
const INTERACT_RADIUS2_U: f64 = INTERACT_RADIUS_U * INTERACT_RADIUS_U;

function in_interact_radius(a: PVF3, b: PVF3): boolean {
  const dx: f64 = pvf3.to_real(pvf3.X(a)) - pvf3.to_real(pvf3.X(b));
  const dy: f64 = pvf3.to_real(pvf3.Y(a)) - pvf3.to_real(pvf3.Y(b));
  return dx * dx + dy * dy <= INTERACT_RADIUS2_U;
}

// Highlighter style
const highlight_style = {
  COLOR: "#ff9f05", // Blender selection orange
  FILL_OPACITY: .6, // solid inner fill
  BORDER_WIDTH: .05, // border thickness
};

const HIGHLIGHT_BOX = new THREE.BoxGeometry(1, 1, 1);

type CellHighlighterProps = {
  map_center: PVF3;
  map_origin: IVec;
};
export function CellHighlighter(props: CellHighlighterProps) {
  const { map_center, map_origin } = props;

  const set_map_center = useMapStore.getState().set_map_center;
  const camera_control_mode = useMapStore((state) => state.camera_control_mode);

  const { raycaster, pointer, camera } = useThree();

  useEffect(() => {
    if (camera_control_mode === CameraControlMode.USER) {
      set_hovered(null);
    }
  }, [camera_control_mode]);

  // High-priority material so the highlighter stays visible above nearby overlays.
  const mat: THREE.ShaderMaterial = useMemo(
    () =>
      new THREE.ShaderMaterial({
        extensions: { derivatives: true } as any, // fwidth 사용
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

// 🖌️ Anti-aliased Step
float aastep(float thres, float value) {
    float afwidth = fwidth(value) * 0.5;
    return smoothstep(thres - afwidth, thres + afwidth, value);
}

void main() {
    // UV 기준 4방향 테두리 계산 (Anti-aliasing 적용)
    float bx = aastep(u_border_width, v_uv.x) * aastep(v_uv.x, 1.0 - u_border_width);
    float by = aastep(u_border_width, v_uv.y) * aastep(v_uv.y, 1.0 - u_border_width);

    // bx * by == 1.0 이면 중앙(Center), 아니면 테두리(Border)
    float is_center = bx * by;
    float is_border = 1.0 - is_center;

    // 테두리는 1.0(불투명), 내부는 u_opacity
    float alpha = mix(u_opacity, 1.0, is_border);
    gl_FragColor = vec4(u_color, alpha);
}`,
        transparent: true,
        side: THREE.DoubleSide,
        depthTest: true,
        // Pull slightly closer than the user marker to avoid overlap ambiguity.
        polygonOffset: true,
        polygonOffsetFactor: -2.0,
        polygonOffsetUnits: -8.0,
      }),
    [],
  );

  const [hovered, set_hovered] = useState<PVF3 | null>(null);

  // Terrain hit test by raymarching against sampled elevation.
  function ray_terrain_delta_at_t(ray: THREE.Ray, t: f64): f64 {
    const hit: THREE.Vector3 = ray.at(t, new THREE.Vector3());

    const p: PVF3 = projection.project({ i: 0n, f: 0n }, hit.x, hit.y, hit.z);
    p.i = ivec.add(p.i, map_origin);

    const ele: f64 = get_ele(p.i, p.f) ?? 0;
    const origin_z: f64 = ivec.to_real(ivec.Z(map_origin));
    const visual_z: f64 = ele - origin_z;
    return hit.z - visual_z;
  }

  function ray_hit_terrain(): PVF3 | null {
    raycaster.setFromCamera(pointer, camera);
    const ray = raycaster.ray;

    let t = 0;
    let hit_t = -1;
    for (let i = 0; i < MAX_STEPS; i++, t += STEP) {
      if (ray_terrain_delta_at_t(ray, t) < HIT_EPS) {
        hit_t = t;
        break;
      }
    }
    if (hit_t === -1) return null;

    let min = hit_t - STEP;
    let max = hit_t;
    for (let i = 0; i < BIN_ITER; i++) {
      const mid = (min + max) * 0.5;
      ray_terrain_delta_at_t(ray, mid) < HIT_EPS ? (max = mid) : (min = mid);
    }

    const hit_three = ray.at(max, new THREE.Vector3());
    const hit: V3 = [hit_three.x, hit_three.y, 0];
    const snapped = floor3(hit, U_PER_CELL);

    const p = projection.project({ i: 0n, f: 0n }, ...snapped);
    p.i = ivec.add(p.i, map_origin);

    return with_ele(p, p.i, p.f);
  }

  function handle_pointer_move(e: ThreeEvent<MouseEvent>) {
    e.stopPropagation();
    const hit: PVF3 | null = ray_hit_terrain();
    if (!hit || !map_center) return set_hovered(null);

    if (!in_interact_radius(hit, map_center)) return set_hovered(null);
    set_hovered(hit);
  }

  function handle_pointer_click(e: ThreeEvent<MouseEvent>) {
    e.stopPropagation();
    if (!hovered) return;
    if (!map_center) return;

    if (in_interact_radius(hovered, map_center)) {
      set_map_center(hovered);
    }
  }

  return (
    <group>
      {/* Input layer */}
      {camera_control_mode !== CameraControlMode.USER && (
        <mesh
          onClick={handle_pointer_click}
          onPointerMove={handle_pointer_move}
          onPointerOut={() => set_hovered(null)}
        >
          <planeGeometry args={[1e5, 1e5]} />
          <meshBasicMaterial transparent opacity={0} depthWrite={false} />
        </mesh>
      )}

      {/* Visual Layer (Highlighter) */}
      {hovered && (
        <AnchorNode i={hovered.i} f={hovered.f}>
          <group position={center3([0, 0, -U_PER_CELL], U_PER_CELL)}>
            <mesh
              geometry={HIGHLIGHT_BOX}
              material={mat}
              scale={U_PER_CELL}
              renderOrder={9999}
            />
          </group>
        </AnchorNode>
      )}
    </group>
  );
}
