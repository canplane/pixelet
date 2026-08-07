// @pxl/web/modules/world/markers/user_marker.tsx

import { useLayoutEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { useFrame } from "@react-three/fiber";

import { BOOL, type f64, type i32, type u32 } from "@xpute/core/kit/word.ts";

import { projection, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { add3, center3n, fill3, floor3n, set3, type V3 } from "@pxl/shared/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import { useTileStore } from "../terrain/tile_store.ts";
import { render_cfg } from "../render/config.ts";
import { get_ele } from "../terrain/use_elevation.ts";

import { AnchorNode } from "../spatial/anchor/node.tsx";
import { WorldThemeKey } from "../render/themes.ts";
import { useThemeKey } from "../render/theme_clock.ts";

const { FOG_FAR } = render_cfg;

type ZoneSample = {
  off: V3;
  borders: [f64, f64, f64, f64]; // top, bottom, right, left
};

// radius in cell units, with a half-diagonal shave so box cells fit the circular zone more cleanly.
// 2 == 16 shrunk to 1/4, then halved again — the .707 shave stays as-is,
// it's a fixed one-cell diagonal correction in CELL units, not a
// world-space dimension that scales with the marker's size.
const ZONE_RADIUS_CELL: f64 = 2 / U_PER_CELL - .707;
const ZONE_RADIUS2_CELL: f64 = ZONE_RADIUS_CELL * ZONE_RADIUS_CELL;

const ZONE_SAMPLES: ZoneSample[] = (() => {
  const range: i32 = Math.ceil(ZONE_RADIUS_CELL);

  const out: ZoneSample[] = [];
  for (let x: i32 = -range; x <= range; x++) {
    for (let y: i32 = -range; y <= range; y++) {
      const dx: f64 = x;
      const dy: f64 = y;
      const dx2: f64 = dx * dx;
      const dy2: f64 = dy * dy;

      if (dx2 + dy2 > ZONE_RADIUS2_CELL) continue;

      const d_top: f64 = dx2 + (dy + 1) * (dy + 1);
      const d_bottom: f64 = dx2 + (dy - 1) * (dy - 1);
      const d_right: f64 = (dx + 1) * (dx + 1) + dy2;
      const d_left: f64 = (dx - 1) * (dx - 1) + dy2;

      out.push({
        off: [dx * U_PER_CELL, dy * U_PER_CELL, -U_PER_CELL],
        borders: [
          BOOL(d_top > ZONE_RADIUS2_CELL),
          BOOL(d_bottom > ZONE_RADIUS2_CELL),
          BOOL(d_right > ZONE_RADIUS2_CELL),
          BOOL(d_left > ZONE_RADIUS2_CELL),
        ],
      });
    }
  }
  return out;
})();

const ZONE_SAMPLE_BOX = new THREE.BoxGeometry(1, 1, 1);
ZONE_SAMPLE_BOX.setAttribute("a_borders", new THREE.InstancedBufferAttribute(new Float32Array(ZONE_SAMPLES.length * 4), 4));

// 1/4 of the original 1 — scaled down together with the rest of the marker.
const BEACON_Z_BIAS: f64 = 0.25;

const zone_style = {
  COLOR: "#4fd1c5",
  FILL_OPACITY: .4,
  BORDER_WIDTH: .085,
} as const;

const beacon_style = {
  COLOR_DAY: "#ff1493",
  COLOR_NIGHT: "#00f2ff",
} as const;
type UserMarkerProps = {
  device_coord: PVF3;
};
export function UserMarker(props: UserMarkerProps) {
  const { device_coord } = props;

  const $v = useRef<V3>([0, 0, 0]);

  const theme_key = useThemeKey();

  const tile = useTileStore((state) => state.get_tile(L3_IDX(device_coord.i)));
  // the zone ring samples get_ele() across its ~63-cell radius, which
  // regularly reaches past the device's own L3 page into neighbors not yet
  // loaded on first mount — those samples fall back to a flat guess and
  // never get revisited, since the effect baking them into the instanced
  // mesh only re-runs when `tile`/`device_coord` change, not when a
  // neighboring page finishes streaming. Subscribing to the store-wide load
  // counter makes the ring self-correct the moment its terrain actually finishes.
  const tile_version = useTileStore((state) => state.tile_version);
  // A placeholder height (sea level, or a "last known good" value) while
  // the device's own page isn't loaded still renders the marker at a
  // guess — floating or sinking wherever real terrain differs. null (not
  // yet grounded) instead gates rendering below rather than papering over
  // it with an estimate.
  const last_known_center_z = useRef<f64 | null>(null);
  const center_pos: V3 = useMemo(() => {
    const pos: V3 = projection.unproject(fill3(0), device_coord.i, device_coord.f);
    const ele: f64 | null = get_ele(device_coord.i, device_coord.f);
    if (ele !== null) last_known_center_z.current = ele;
    pos[2] = last_known_center_z.current ?? 0;
    return floor3n(pos, U_PER_CELL);
  }, [device_coord, tile]);

  const has_ground: boolean = last_known_center_z.current !== null;

  const center_coord = useMemo(() => {
    return projection.project({ i: 0n, f: 0n }, ...center_pos);
  }, [center_pos]);

  const zone_ref = useRef<THREE.InstancedMesh | null>(null);
  const beacon_ref = useRef<THREE.Mesh>(null);

  const is_night: boolean = theme_key === WorldThemeKey.NIGHT;
  const beacon_color: string = is_night ? beacon_style.COLOR_NIGHT : beacon_style.COLOR_DAY;

  const mat: THREE.ShaderMaterial = useMemo(
    () =>
      new THREE.ShaderMaterial({
        extensions: { derivatives: true } as any,
        uniforms: {
          u_color: { value: new THREE.Color(zone_style.COLOR) },
          u_fill_opacity: { value: zone_style.FILL_OPACITY },
          u_border_width: { value: zone_style.BORDER_WIDTH },
        },
        vertexShader: `attribute vec4 a_borders;
varying vec2 v_uv;
varying vec4 v_borders;

void main() {
  v_uv = uv;
  v_borders = a_borders;
  gl_Position = projectionMatrix * modelViewMatrix * instanceMatrix * vec4(position, 1.0);
}`,
        fragmentShader: `
uniform vec3 u_color;
uniform float u_fill_opacity;
uniform float u_border_width;

varying vec2 v_uv;
varying vec4 v_borders;

float aastep(float thres, float value) {
  float afwidth = fwidth(value) * 0.5;
  return smoothstep(thres - afwidth, thres + afwidth, value);
}

void main() {
  vec4 base_color = vec4(u_color, u_fill_opacity);
  vec4 border_color = vec4(u_color, 1.0);

  float border_top    = aastep(1.0 - u_border_width, v_uv.y) * v_borders.x;
  float border_bottom = (1.0 - aastep(u_border_width, v_uv.y)) * v_borders.y;
  float border_right  = aastep(1.0 - u_border_width, v_uv.x) * v_borders.z;
  float border_left   = (1.0 - aastep(u_border_width, v_uv.x)) * v_borders.w;

  float border_strength = max(max(border_top, border_bottom), max(border_left, border_right));
  gl_FragColor = mix(base_color, border_color, border_strength);
}`,
        transparent: true,
        depthWrite: false,
        depthTest: true,
        side: THREE.DoubleSide,
        polygonOffset: true,
        polygonOffsetFactor: -1.0,
        polygonOffsetUnits: -4.0,
      }),
    [],
  );

  const temp_obj = useMemo(() => new THREE.Object3D(), []);

  useLayoutEffect(() => {
    const mesh = zone_ref.current;
    if (!mesh) return;

    const v: V3 = $v.current;

    mesh.count = ZONE_SAMPLES.length;
    const a_borders = mesh.geometry.attributes.a_borders as THREE.InstancedBufferAttribute;

    for (let i: u32 = 0; i < ZONE_SAMPLES.length; i++) {
      const { off, borders } = ZONE_SAMPLES[i];

      add3(center_pos, off, v);
      const coord: PVF3 = projection.project({ i: 0n, f: 0n }, ...v);
      // same reasoning as last_known_center_z: a sample can land on a
      // neighboring page not loaded yet — falling back to absolute 0 there
      // would sink/launch that instance away from the rest of the zone, so
      // assume it's flat (same height as center) until the real sample streams in.
      const delta_ele: f64 = (get_ele(coord.i, coord.f) ?? center_pos[2]) - center_pos[2];

      set3(off[0], off[1], off[2] + delta_ele, v);
      center3n(v, U_PER_CELL, v);
      temp_obj.position.set(v[0], v[1], v[2]);
      temp_obj.scale.set(U_PER_CELL, U_PER_CELL, U_PER_CELL);
      temp_obj.updateMatrix();
      mesh.setMatrixAt(i, temp_obj.matrix);

      a_borders.array[i * 4 + 0] = borders[0];
      a_borders.array[i * 4 + 1] = borders[1];
      a_borders.array[i * 4 + 2] = borders[2];
      a_borders.array[i * 4 + 3] = borders[3];
    }

    mesh.instanceMatrix.needsUpdate = true;
    a_borders.needsUpdate = true;
  }, [center_pos, temp_obj, tile_version]);

  const $beacon_pos = useRef(new THREE.Vector3());

  useFrame((state) => {
    if (!beacon_ref.current) return;

    beacon_ref.current.getWorldPosition($beacon_pos.current);

    const dist: f64 = state.camera.position.distanceTo($beacon_pos.current);
    const visible: boolean = dist < FOG_FAR;

    beacon_ref.current.visible = visible;
    if (zone_ref.current) zone_ref.current.visible = visible;

    if (!visible) return;

    const t: f64 = state.clock.getElapsedTime();
    beacon_ref.current.rotation.z = t * 1.2;
    // bob amplitude, also 1/4 of the original 0.2.
    beacon_ref.current.position.z = BEACON_Z_BIAS + Math.sin(t * 2) * (0.05);
  });

  // nothing to ground the marker to yet — see last_known_center_z above.
  // briefly absent beats briefly floating/sunk at a guessed height.
  if (!has_ground) return null;

  return (
    <AnchorNode i={center_coord.i} f={center_coord.f}>
      <instancedMesh
        ref={zone_ref}
        args={[ZONE_SAMPLE_BOX, mat, ZONE_SAMPLES.length]}
        frustumCulled={false}
        renderOrder={5}
      />

      <mesh
        ref={beacon_ref}
        // U_PER_CELL, not 1 — every other position here (center_pos, each
        // zone tile, cell_highlighter.tsx's POS) snaps to that same cell
        // grid. Snapping to the 1-unit grid instead put the beacon up to 2
        // cells away from where center_pos/center_coord actually anchor —
        // visible as sitting off-center from the zone ring.
        position={center3n([0, 0, BEACON_Z_BIAS], U_PER_CELL)}
      >
        {/* 1/4 of the original 0.6. */}
        <octahedronGeometry args={[0.15, 0]} />
        <meshStandardMaterial
          color={beacon_color}
          emissive={beacon_color}
          emissiveIntensity={is_night ? 2 : 0.5}
          roughness={0.2}
          metalness={0.8}
          flatShading
        />
        <pointLight
          distance={ZONE_RADIUS_CELL * U_PER_CELL}
          intensity={is_night ? 10 : 3}
          color={beacon_color}
        />
      </mesh>
    </AnchorNode>
  );
}
