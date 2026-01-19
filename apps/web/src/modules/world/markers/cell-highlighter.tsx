// @pxl/web/modules/world/markers/cell-highlighter.tsx

import { useEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import type { ThreeEvent } from "@react-three/fiber";
import { useThree } from "@react-three/fiber";

import type { f64, u32 } from "@xpute/spec/lib/kit/word.ts";

import { type IVec, ivec, projection, type PVF3, pvf3 } from "@pxl/spec/space/pvf3/mod.ts";
import { center3, fill3, floor3, type V3 } from "@pxl/spec/space/v3.ts";
import { from3, to3 } from "@pxl/spec/space/adapter/three.ts";

import { U_PER_CELL } from "@pxl/bake/terrain/constants.ts";

import { AnchorNode } from "@/modules/world/spatial/anchor/node.tsx";

import { useMapStore } from "@/modules/map/navigation/map-store.ts";
import { get_ele, with_ele } from "../terrain/use-elevation.ts";

// raymarching configuration
const STEP: f64 = 2 * U_PER_CELL;
const MAX_STEPS: u32 = 200;
const BIN_ITER: u32 = 6;
const HIT_EPS: f64 = 0.1 * U_PER_CELL;

// half-diagonal inset so snapped cell boxes stay inside the intended circular range
const INTERACT_RADIUS_U: f64 = (32 - .707) * U_PER_CELL;
const INTERACT_RADIUS2_U: f64 = INTERACT_RADIUS_U * INTERACT_RADIUS_U;

function in_interact_radius(a: PVF3, b: PVF3): boolean {
  const dx: f64 = pvf3.to_real(pvf3.X(a)) - pvf3.to_real(pvf3.X(b));
  const dy: f64 = pvf3.to_real(pvf3.Y(a)) - pvf3.to_real(pvf3.Y(b));
  return dx * dx + dy * dy <= INTERACT_RADIUS2_U;
}

// highlight style
const style = {
  COLOR: "#ff9f05", // Blender Selection Orange
  FILL_OPACITY: .6, // 진한 내부 채움
  BORDER_WIDTH: .05, // 외곽선 두께
};

const BOX = new THREE.BoxGeometry(1, 1, 1);

type CellHighlighterProps = {
  map_center: PVF3;
  map_origin: IVec;
};
export function CellHighlighter(props: CellHighlighterProps) {
  const { map_center, map_origin } = props;

  const set_map_center = useMapStore.getState().set_map_center;

  const { raycaster, pointer, camera } = useThree();

  // Refs (Animation Loop 등에서 최신 상태 접근용)
  const state_ref = useRef({ map_center, map_origin });
  useEffect(() => {
    state_ref.current = { map_center, map_origin };
  }, [map_center, map_origin]);

  // 1️⃣ [Material] High-Priority Shader Material
  const mat: THREE.ShaderMaterial = useMemo(
    () =>
      new THREE.ShaderMaterial({
        extensions: { derivatives: true } as any, // fwidth 사용
        uniforms: {
          u_color: { value: new THREE.Color(style.COLOR) },
          u_opacity: { value: style.FILL_OPACITY },
          u_border_width: { value: style.BORDER_WIDTH },
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
        // 🛡️ [Priority] 가장 중요함: UserMarker(-1.0)보다 더 강하게 당김 (-2.0)
        // 이 설정 덕분에 마커와 겹칠 때 항상 이 하이라이터가 위에 그려집니다.
        polygonOffset: true,
        polygonOffsetFactor: -2.0,
        polygonOffsetUnits: -8.0,
      }),
    [],
  );

  const [hovered, set_hovered] = useState<PVF3 | null>(null);

  // 2️⃣ [Raymarching] 지형 충돌 감지 (Logic)
  function ray_terrain_delta_at_t(ray: THREE.Ray, t: f64): f64 {
    const hit_three: THREE.Vector3 = ray.at(t, new THREE.Vector3());
    const hit: V3 = from3([hit_three.x, hit_three.y, hit_three.z]);

    const p: PVF3 = projection.project({ i: 0n, f: 0n }, ...hit);
    p.i = ivec.add(p.i, state_ref.current.map_origin);

    const ele: f64 = get_ele(p.i, p.f) ?? 0;
    const origin_z: f64 = ivec.to_real(ivec.Z(state_ref.current.map_origin));
    const visual_z: f64 = ele - origin_z;
    return hit[2] - visual_z;
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
    const hit = from3([0, 0, 0], [hit_three.x, 0, hit_three.z]);
    const snapped = floor3(hit, fill3(U_PER_CELL));

    const p = projection.project({ i: 0n, f: 0n }, ...snapped);
    p.i = ivec.add(p.i, state_ref.current.map_origin);

    return with_ele(p, p.i, p.f);
  }

  function handle_pointer_move(e: ThreeEvent<MouseEvent>) {
    e.stopPropagation();
    const hit: PVF3 | null = ray_hit_terrain();
    if (!hit || !state_ref.current.map_center) return set_hovered(null);

    if (!in_interact_radius(hit, state_ref.current.map_center)) return set_hovered(null);
    set_hovered(hit);
  }

  function handle_pointer_click(e: ThreeEvent<MouseEvent>) {
    e.stopPropagation();
    if (!hovered) return;
    if (!state_ref.current.map_center) return;

    if (in_interact_radius(hovered, state_ref.current.map_center)) {
      set_map_center(hovered);
    }
  }

  return (
    <group>
      {/* Input Layer (Invisible Plane) */}
      <mesh rotation={[-Math.PI / 2, 0, 0]} onClick={handle_pointer_click} onPointerMove={handle_pointer_move} onPointerOut={() => set_hovered(null)}>
        <planeGeometry args={[1e5, 1e5]} />
        {/* <meshBasicMaterial visible={false} /> */}
        <meshBasicMaterial transparent opacity={0} depthWrite={false} />
      </mesh>

      {/* Visual Layer (Highlighter) */}
      {hovered && (
        <AnchorNode i={hovered.i} f={hovered.f}>
          <group position={to3(center3([0, 0, -U_PER_CELL], fill3(U_PER_CELL)))}>
            <mesh
              geometry={BOX}
              material={mat}
              scale={fill3(U_PER_CELL)}
              renderOrder={9999}
            />
          </group>
        </AnchorNode>
      )}
    </group>
  );
}
