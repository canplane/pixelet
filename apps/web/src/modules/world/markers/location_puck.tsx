// @pxl/web/modules/world/markers/location_puck.tsx

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { useFrame } from "@react-three/fiber";

import { BOOL, type f64, type i32, type u32 } from "@xpute/core/kit/word.ts";

import { ivec, projection, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { add3, center3n, fill3, floor3n, set3, type V3 } from "@pxl/shared/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import { register_tile_interest, useTileStore } from "../terrain/store.ts";
import { render_cfg } from "../environment/config.ts";
import { get_ele } from "../terrain/elevation.ts";

import { AnchorNode } from "../anchor/node.tsx";
import { WorldThemeKey } from "../environment/themes.ts";
import { useThemeKey } from "../environment/theme_clock.ts";
import { marker_colors } from "./style.ts";

const { FOG_FAR } = render_cfg;

type ZoneSample = {
  off: V3;
  borders: [f64, f64, f64, f64]; // top, bottom, right, left
};

// radius in cell units, with a half-diagonal shave so box cells fit the circular zone more cleanly.
// 4 == 2 doubled — when CELL_SHAMT doubled U_PER_CELL (0.5u -> 1u), this
// world-space numerator stayed as-is (by design — see the .707 note below),
// which kept the zone's world-space footprint fixed but rendered it with
// half as many, twice-as-big cells: same size, visibly blockier. Doubling
// the numerator instead doubles the zone's actual world-space radius, which
// reads as "bigger, not just blockier" and is what was asked for. The .707
// shave stays as-is — it's a fixed one-cell diagonal correction in CELL
// units, not a world-space dimension that scales with the marker's size.
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

// 1/2 of the original 1 — doubled back up from 0.25 (1/4) along with the
// zone ring and octahedron radius, so the beacon floats proportionally
// above the now-larger ring instead of sitting low relative to it.
const BEACON_Z_BIAS: f64 = 0.5;

/**
 * Exponential rate the beacon closes the gap to a new fix at, per second:
 * `1 - exp(-RATE * dt)`, so it is frame-rate independent rather than "some
 * fraction per frame".
 *
 * Geolocation delivers roughly one fix a second, and the beacon reads the
 * raw store (use_map_navigation.ts's device_coord, not follow mode's own
 * EMA'd copy), so without this it teleports once per fix — a few metres at
 * walking pace, more at a run. 6 closes ~95% of the gap in half a second:
 * fast enough that the beacon is never meaningfully behind where the device
 * says it is, slow enough to read as travel rather than a cut.
 *
 * Only the horizontal axes are smoothed. Z is terrain elevation, which
 * changes when the ground under the marker changes, and easing that would
 * sink the beacon into a slope on the way up it.
 */
const BEACON_SMOOTH_RATE: f64 = 6;

/**
 * Past this much of a jump the beacon cuts instead of sliding, in world
 * units. The zone ring's own radius is the threshold because it is what the
 * marker visually occupies: inside it, a slide reads as the marker moving
 * within its own footprint; beyond it, the fix has landed somewhere the
 * previous position had nothing to do with (a first fix, a return from a
 * dropout, an accuracy correction jumping a block over) and sliding across
 * that gap would draw a trip that never happened.
 */
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

  // Holds the last known device position across a transient GPS dropout
  // (device_coord going null — e.g. right after a tab resumes and the
  // sensor is still reacquiring). Previously this component was fully
  // unmounted/remounted by its caller on every such blip, tearing down and
  // rebuilding the zone's InstancedMesh (and its shared singleton geometry)
  // each time — the repeated remounts, not the position math itself (which
  // always computed correctly), were the source of a "stacked/cube"
  // rendering glitch seen right after tab-away-and-back. Same "hold the
  // last good value" idiom as last_known_center_z below, just one level up.
  const last_known_device_coord = useRef<PVF3 | null>(null);
  if (device_coord !== null) last_known_device_coord.current = device_coord;
  const effective_device_coord: PVF3 | null = last_known_device_coord.current;

  const own_page_idx = effective_device_coord !== null ? L3_IDX(effective_device_coord.i) : 0n;
  const tile = useTileStore((state) => state.get_tile(own_page_idx));

  const [recompute_tick, set_recompute_tick] = useState(0);
  // the zone ring samples get_ele() across its own L3 page plus its
  // immediate neighbors, which can reach past the device's own page into a
  // neighbor not yet loaded on first mount — those samples fall back to a
  // flat guess and never get revisited unless something tells the ring a
  // page it originally missed has since streamed in. Registering interest
  // in just that 3x3 page neighborhood (rather than every page load
  // anywhere in the world) means the ring only recomputes when a page it
  // could actually be sampling from finishes loading.
  useEffect(() => {
    return register_tile_interest(
      (page_idx) => Number(ivec.max_comp(ivec.abs(ivec.sub(page_idx, own_page_idx)))) <= 1,
      () => set_recompute_tick((t) => t + 1),
    );
  }, [own_page_idx]);
  // A placeholder height (sea level, or a "last known good" value) while
  // the device's own page isn't loaded still renders the marker at a
  // guess — floating or sinking wherever real terrain differs. null (not
  // yet grounded) instead gates rendering below rather than papering over
  // it with an estimate.
  const last_known_center_z = useRef<f64 | null>(null);
  const center_pos: V3 = useMemo(() => {
    if (effective_device_coord === null) return fill3(0);
    const pos: V3 = projection.unproject(fill3(0), effective_device_coord.i, effective_device_coord.f);
    const ele: f64 | null = get_ele(effective_device_coord.i, effective_device_coord.f);
    if (ele !== null) last_known_center_z.current = ele;
    pos[2] = last_known_center_z.current ?? 0;
    return floor3n(pos, U_PER_CELL);
  }, [effective_device_coord, tile]);

  // nothing to place the marker at yet at all (no fix has ever landed), or
  // nothing to ground it to (same last_known_center_z reasoning as before).
  const has_ground: boolean = effective_device_coord !== null && last_known_center_z.current !== null;

  const center_coord = useMemo(() => {
    return projection.project({ i: 0n, f: 0n }, ...center_pos);
  }, [center_pos]);

  // Where the device actually is *within* its cell, on the horizontal axes
  // only. center_pos floors to the cell grid because the zone ring is built
  // out of cell-sized boxes and has to line up with them; the beacon is a
  // free-floating octahedron under no such constraint, and snapping it too
  // quantized the reported position — walking across a cell moved the
  // beacon not at all, then a whole cell at once. Adding the remainder back
  // puts it at the real coordinate while the ring underneath stays snapped.
  //
  // Z is deliberately not part of this: it is terrain elevation plus the
  // bob, both written every frame in useFrame below, so there is nothing
  // here for a sub-cell correction to contribute.
  const beacon_offset: [f64, f64] = useMemo(() => {
    if (effective_device_coord === null) return [0, 0];
    const pos: V3 = projection.unproject(fill3(0), effective_device_coord.i, effective_device_coord.f);
    return [pos[0] - center_pos[0], pos[1] - center_pos[1]];
  }, [effective_device_coord, center_pos]);

  const zone_ref = useRef<THREE.InstancedMesh | null>(null);
  // AnchorNode is a two-pass portal (target starts null, resolves via its
  // own useLayoutEffect — see node.tsx): zone_ref's instancedMesh doesn't
  // actually exist in the scene until that second pass commits. The
  // matrix-writing effect below is keyed on [center_pos, temp_obj,
  // recompute_tick] — none of which change between AnchorNode's first and
  // second pass — so a plain useRef here meant that effect's one real,
  // useful run (the one where zone_ref.current is finally non-null) never
  // happened: it ran once early with mesh=null (a no-op, guarded away) and
  // never got a reason to run again. The instancedMesh then kept
  // three.js's default identity instanceMatrix for every one of its
  // ZONE_SAMPLES instances — all of them sitting exactly on top of each
  // other at local (0,0,0), which reads as one solid stacked cube instead
  // of a ring. zone_mesh_ready flips true the moment the mesh actually
  // attaches, giving the effect below a real dependency change to fire on.
  const [zone_mesh_ready, set_zone_mesh_ready] = useState(false);
  const set_zone_ref = useCallback((mesh: THREE.InstancedMesh | null) => {
    zone_ref.current = mesh;
    set_zone_mesh_ready(mesh !== null);
  }, []);
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
  }, [center_pos, temp_obj, recompute_tick, zone_mesh_ready]);

  const $beacon_pos = useRef(new THREE.Vector3());

  /** Where the beacon is actually drawn, in the same local frame as
   * beacon_offset. null until the first fix seeds it. */
  const shown_offset = useRef<[f64, f64] | null>(null);

  /**
   * Rebases shown_offset whenever the anchor moves.
   *
   * center_pos floors to the cell grid, so crossing a cell boundary jumps it
   * a whole cell while the true position moved a fraction of one. The world
   * position is continuous across that; the *local* offset is not — it drops
   * by a cell at the same instant the anchor gains one. Easing the local
   * value directly would therefore play that discontinuity as a visible
   * slide backwards across the cell, once per boundary, in the opposite
   * direction to travel. Subtracting the anchor's own delta keeps what is
   * being eased anchored to the world instead.
   */
  const prev_center = useRef<V3 | null>(null);
  useLayoutEffect(() => {
    const prev = prev_center.current;
    if (prev !== null && shown_offset.current !== null) {
      shown_offset.current[0] -= center_pos[0] - prev[0];
      shown_offset.current[1] -= center_pos[1] - prev[1];
    }
    prev_center.current = [center_pos[0], center_pos[1], center_pos[2]];
  }, [center_pos]);

  useFrame((state, delta) => {
    if (!beacon_ref.current) return;

    beacon_ref.current.getWorldPosition($beacon_pos.current);

    const dist: f64 = state.camera.position.distanceTo($beacon_pos.current);
    const visible: boolean = dist < FOG_FAR;

    beacon_ref.current.visible = visible;
    if (zone_ref.current) zone_ref.current.visible = visible;

    if (!visible) return;

    // Ease toward the fix rather than cutting to it — see
    // BEACON_SMOOTH_RATE. Written here, not through the mesh's `position`
    // prop, so React re-rendering on a new fix never fights the frame that
    // is mid-slide.
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
    // bob amplitude — doubled back up too, see BEACON_Z_BIAS above.
    beacon_ref.current.position.z = BEACON_Z_BIAS + Math.sin(t * 2) * (0.1);

    // this spin/bob never converges by design (it's meant to animate
    // forever while the marker is on screen) — under frameloop="demand"
    // (scene.tsx) that means it has to keep asking for a next frame itself,
    // every frame, for as long as it stays visible. Unlike the damped
    // camera/frame hooks, there's no settling point here to stop at.
    state.invalidate();
  });

  // nothing to ground the marker to yet — see last_known_center_z above.
  // briefly absent beats briefly floating/sunk at a guessed height.
  if (!has_ground) return null;

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
        // Placeholder for the frame before useFrame first runs; it owns all
        // three axes from then on — x/y easing toward beacon_offset, z the
        // terrain-relative bob.
        position={[beacon_offset[0], beacon_offset[1], BEACON_Z_BIAS]}
      >
        {
          /* 1/2 of the original 0.6 — bumped back up from 0.15 (1/4) so the
        beacon doesn't look undersized against the now-bigger zone ring. */
        }
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
