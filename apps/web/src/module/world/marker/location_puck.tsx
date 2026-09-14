// @pxl/web/module/world/marker/location_puck.tsx

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";

import { BOOL, type f64, type i32, type u32 } from "@xpute/core/kit/word.ts";

import { ivec, projection, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { add3, center3n, fill3, floor3n, set3, type V3 } from "@pxl/shared/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/shared/bake/constant.ts";
import { register_tile_interest, useTileStore } from "../terrain/store.ts";
import { render_cfg } from "../environment/config.ts";
import { get_ele } from "../terrain/elevation.ts";

import { AnchorNode } from "../anchor/node.tsx";
import { WorldThemeKey } from "../environment/theme.ts";
import { useThemeKey } from "../environment/theme_clock.ts";
import { marker_colors } from "./style.ts";
import { useTick } from "@/module/world/sched/tick.ts";
import { CORE_PXL, FIXED_ALLOC } from "@/render/layer/pipeline/core_pool.ts";
import { useGpuDrawsChunks } from "@/render/gpu/stage.tsx";
import { request_world_tick } from "../change.ts";

const { FOG_FAR } = render_cfg;

type ZoneSample = {
  off: V3;
  borders: [f64, f64, f64, f64]; // top, bottom, right, left
};

// Numerator (4) is a world-space distance, not a cell count — it fixes the zone's
// footprint size independent of U_PER_CELL. The .707 shave is a fixed one-cell
// diagonal correction in CELL units, so it does not scale with U_PER_CELL either.
const ZONE_RADIUS_CELL: f64 = 4 / U_PER_CELL - .707;
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

const BEACON_Z_BIAS: f64 = 0.5;

// Frame-rate independent exponential smoothing: `1 - exp(-RATE * dt)`. Only x/y are
// eased — Z is terrain elevation, and easing it would sink the beacon into a slope.
const BEACON_SMOOTH_RATE: f64 = 6;

const BEACON_SNAP_DIST2: f64 = (ZONE_RADIUS_CELL * U_PER_CELL) ** 2;

const zone_style = {
  COLOR: marker_colors.ACTION,
  FILL_OPACITY: .4,
  BORDER_WIDTH: .04,
} as const;

const beacon_style = {
  COLOR_DAY: "#ff1493",
  COLOR_NIGHT: "#00f2ff",
} as const;
type LocationPuckProps = {
  device_coord: PVF3 | null;
};
export function LocationPuck(props: LocationPuckProps) {
  const { device_coord } = props;

  const $v = useRef<V3>([0, 0, 0]);

  const theme_key = useThemeKey();

  // Holds the last known device position across a transient GPS dropout (device_coord
  // going null). Without this, the caller previously remounted this component on every
  // such blip, tearing down and rebuilding the zone's InstancedMesh — that remount, not
  // the position math, was the source of a "stacked/cube" rendering glitch after tab-away-and-back.
  const last_known_device_coord = useRef<PVF3 | null>(null);
  if (device_coord !== null) last_known_device_coord.current = device_coord;
  const effective_device_coord: PVF3 | null = last_known_device_coord.current;

  const own_page_idx = effective_device_coord !== null ? L3_IDX(effective_device_coord.i) : 0n;
  const tile = useTileStore((state) => state.get_tile(own_page_idx));

  const [recompute_tick, set_recompute_tick] = useState(0);
  // The zone ring samples get_ele() across its own L3 page plus its immediate
  // neighbors, which can reach a page not yet loaded on first mount. Without this,
  // those samples would fall back to a flat guess and never get revisited once the
  // missing page streams in.
  useEffect(() => {
    return register_tile_interest(
      (page_idx) => Number(ivec.max_comp(ivec.abs(ivec.sub(page_idx, own_page_idx)))) <= 1,
      () => set_recompute_tick((t) => t + 1),
    );
  }, [own_page_idx]);
  // null (not yet grounded) gates rendering below instead of rendering the marker at
  // a guessed height while the device's own page isn't loaded yet.
  const last_known_center_z = useRef<f64 | null>(null);
  const center_pos: V3 = useMemo(() => {
    if (effective_device_coord === null) return fill3(0);
    const pos: V3 = projection.unproject(fill3(0), effective_device_coord.i, effective_device_coord.f);
    const ele: f64 | null = get_ele(effective_device_coord.i, effective_device_coord.f);
    if (ele !== null) last_known_center_z.current = ele;
    pos[2] = last_known_center_z.current ?? 0;
    return floor3n(pos, U_PER_CELL);
  }, [effective_device_coord, tile]);

  const has_ground: boolean = effective_device_coord !== null && last_known_center_z.current !== null;

  const center_coord = useMemo(() => {
    return projection.project({ i: 0n, f: 0n }, ...center_pos);
  }, [center_pos]);

  // center_pos floors to the cell grid to line up with the zone ring's cell-sized
  // boxes; the beacon has no such constraint, so this adds the remainder back —
  // otherwise it would quantize to whole-cell jumps instead of moving smoothly.
  const beacon_offset: [f64, f64] = useMemo(() => {
    if (effective_device_coord === null) return [0, 0];
    const pos: V3 = projection.unproject(fill3(0), effective_device_coord.i, effective_device_coord.f);
    return [pos[0] - center_pos[0], pos[1] - center_pos[1]];
  }, [effective_device_coord, center_pos]);

  const zone_ref = useRef<THREE.InstancedMesh | null>(null);
  // AnchorNode is a two-pass portal (node.tsx): the instancedMesh doesn't exist in the
  // scene until the second pass commits, after the matrix-writing effect below's deps
  // have already settled. A plain useRef here means that effect never gets a reason to
  // re-run once the mesh attaches, leaving every instance at three.js's default identity
  // matrix — all stacked at local (0,0,0), rendering as one solid cube instead of a ring.
  // zone_mesh_ready gives the effect a real dependency change to fire on when that happens.
  const [zone_mesh_ready, set_zone_mesh_ready] = useState(false);
  const set_zone_ref = useCallback((mesh: THREE.InstancedMesh | null) => {
    zone_ref.current = mesh;
    set_zone_mesh_ready(mesh !== null);
  }, []);
  const beacon_ref = useRef<THREE.Mesh>(null);

  const is_night: boolean = theme_key === WorldThemeKey.NIGHT;
  const beacon_color: string = is_night ? beacon_style.COLOR_NIGHT : beacon_style.COLOR_DAY;

  // Deliberately unlit (not chunk_stage.tsx's/hover_cursor.tsx's own
  // real-lit approach) — tried the lit version (2026-09-03) and reverted:
  // this zone's whole job is standing out as a clear "here's your
  // interaction radius" indicator, not reading as a real painted-on
  // surface. Blending into the scene's actual lighting made it LESS
  // legible, the opposite of a location marker's actual requirement —
  // unlike hover_cursor.tsx's ghost, which wants to look like a real
  // preview of a real thing, this has no "real thing" to resemble.
  const mat: THREE.ShaderMaterial = useMemo(
    () =>
      new THREE.ShaderMaterial({
        // `derivatives` isn't in this three version's own ShaderMaterialParameters['extensions']
        // type (WebGL2 has GL_OES_standard_derivatives as core, so the flag itself is stale) —
        // kept at runtime rather than removed since dropping it wasn't verified to be a no-op.
        // deno-lint-ignore no-explicit-any
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

  // Through the WebGPU renderer the puck is the core's (crates/pxl-core/src/
  // marker.rs): this still decides where the device is, which cells the zone
  // covers at what height and where the beacon should stand; the core holds
  // the zone, eases, spins and bobs the beacon, and draws both.
  const gpu = useGpuDrawsChunks();
  useLayoutEffect(() => {
    const p = CORE_PXL;
    if (!p || !gpu) return;
    if (!has_ground) {
      p.pxl_puck_clear();
      return;
    }
    const v: V3 = $v.current;
    const lane = FIXED_ALLOC.take(Float32Array, ZONE_SAMPLES.length * 7);
    for (let i: u32 = 0; i < ZONE_SAMPLES.length; i++) {
      const { off, borders } = ZONE_SAMPLES[i];
      add3(center_pos, off, v);
      const coord: PVF3 = projection.project({ i: 0n, f: 0n }, ...v);
      const delta_ele: f64 = (get_ele(coord.i, coord.f) ?? center_pos[2]) - center_pos[2];
      set3(off[0], off[1], off[2] + delta_ele, v);
      center3n(v, U_PER_CELL, v);
      lane.set([v[0], v[1], v[2], borders[0], borders[1], borders[2], borders[3]], i * 7);
    }
    p.pxl_puck_zone(center_pos[0], center_pos[1], center_pos[2], ZONE_SAMPLES.length, lane.byteOffset, ZONE_RADIUS_CELL * U_PER_CELL, FOG_FAR);
    FIXED_ALLOC.give(lane);
    request_world_tick();
  }, [gpu, has_ground, center_pos, recompute_tick]);
  useEffect(() => {
    if (gpu) CORE_PXL?.pxl_puck_beacon(beacon_offset[0], beacon_offset[1], is_night ? 1 : 0);
  }, [gpu, beacon_offset, is_night, center_pos]);
  useEffect(() => () => CORE_PXL?.pxl_puck_clear(), []);

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
      // Falls back to flat (center height) rather than 0 when a sample lands on an
      // unloaded neighboring page — absolute 0 would sink/launch that instance away
      // from the rest of the zone.
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
  }, [center_pos, temp_obj, recompute_tick, zone_mesh_ready]);

  const $beacon_pos = useRef(new THREE.Vector3());

  const shown_offset = useRef<[f64, f64] | null>(null);

  // Rebases shown_offset whenever the anchor moves. center_pos floors to the cell
  // grid, so crossing a cell boundary jumps it a whole cell while the true position
  // moved a fraction of one — without subtracting the anchor's own delta here, easing
  // the local offset directly would play that discontinuity as a visible backward slide.
  const prev_center = useRef<V3 | null>(null);
  useLayoutEffect(() => {
    const prev = prev_center.current;
    if (prev !== null && shown_offset.current !== null) {
      shown_offset.current[0] -= center_pos[0] - prev[0];
      shown_offset.current[1] -= center_pos[1] - prev[1];
    }
    prev_center.current = [center_pos[0], center_pos[1], center_pos[2]];
  }, [center_pos]);

  useTick("interaction", ({ state, delta }) => {
    if (!beacon_ref.current) return;

    beacon_ref.current.getWorldPosition($beacon_pos.current);

    const dist: f64 = state.camera.position.distanceTo($beacon_pos.current);
    const visible: boolean = dist < FOG_FAR;

    beacon_ref.current.visible = visible;
    if (zone_ref.current) zone_ref.current.visible = visible;

    if (!visible) return;

    // Written directly here, not through the mesh's `position` prop, so a React
    // re-render on a new fix never fights the frame that is mid-slide.
    if (shown_offset.current === null) shown_offset.current = [beacon_offset[0], beacon_offset[1]];
    const shown: [f64, f64] = shown_offset.current;
    const gap_x: f64 = beacon_offset[0] - shown[0];
    const gap_y: f64 = beacon_offset[1] - shown[1];
    if (gap_x * gap_x + gap_y * gap_y > BEACON_SNAP_DIST2) {
      shown[0] = beacon_offset[0];
      shown[1] = beacon_offset[1];
    } else {
      const alpha: f64 = 1 - Math.exp(-BEACON_SMOOTH_RATE * delta);
      shown[0] += gap_x * alpha;
      shown[1] += gap_y * alpha;
    }
    beacon_ref.current.position.x = shown[0];
    beacon_ref.current.position.y = shown[1];

    const t: f64 = state.clock.getElapsedTime();
    beacon_ref.current.rotation.z = t * 1.2;
    beacon_ref.current.position.z = BEACON_Z_BIAS + Math.sin(t * 2) * (0.1);

    // This spin/bob never converges by design, so under frameloop="demand" (scene.tsx)
    // it must keep requesting a next frame itself for as long as it stays visible —
    // unlike the damped camera/frame hooks, there's no settling point to stop at.
    state.invalidate();
  }, "puck");

  if (!has_ground || gpu) return null;

  return (
    <AnchorNode i={center_coord.i} f={center_coord.f}>
      <instancedMesh
        ref={set_zone_ref}
        args={[ZONE_SAMPLE_BOX, mat, ZONE_SAMPLES.length]}
        frustumCulled={false}
        renderOrder={5}
      />

      <mesh
        ref={beacon_ref}
        position={[beacon_offset[0], beacon_offset[1], BEACON_Z_BIAS]}
      >
        <octahedronGeometry args={[0.3, 0]} />
        <meshStandardMaterial
          color={beacon_color}
          emissive={beacon_color}
          emissiveIntensity={is_night ? 1 : 0.25}
          roughness={0.2}
          metalness={0.8}
          flatShading
        />
        <pointLight
          distance={ZONE_RADIUS_CELL * U_PER_CELL * 2.5}
          intensity={1}
          color={beacon_color}
        />
      </mesh>
    </AnchorNode>
  );
}
