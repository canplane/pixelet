// @pxl/web/modules/world/markers/user-marker/object.ts

import * as THREE from "three/webgpu";

import { BOOL, type f64, type i32, type u32 } from "@xpute/spec/lib/kit/word.ts";

import type { PVF3 } from "@pxl/spec/space/pvf3/mod.ts";
import { ivec, projection, pvf3 } from "@pxl/spec/space/pvf3/mod.ts";
import { L4_BASE, L4_IDX } from "@pxl/spec/space/indexer.ts";
import { add3, center3n, fill3, floor3n, set3, type V3 } from "@pxl/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/bake/terrain/constants.ts";
import { render_cfg } from "../../render/config.ts";
import { get_ele } from "../../terrain/use-elevation.ts";

import type { AnchorRegistry } from "../../spatial/anchor/registry.ts";
import { create_zone_material } from "./material.ts";

const { FOG_FAR } = render_cfg;

type ZoneSample = {
  off: V3;
  borders: [f64, f64, f64, f64]; // top, bottom, right, left
};

// Radius in cell units, with a half-diagonal shave so box cells fit the circular zone more cleanly.
const ZONE_RADIUS_CELL: f64 = 16 / U_PER_CELL - .707;
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
ZONE_SAMPLE_BOX.setAttribute(
  "a_borders",
  new THREE.InstancedBufferAttribute(new Float32Array(ZONE_SAMPLES.length * 4), 4),
);

const BEACON_Z_BIAS: f64 = 1;

const beacon_style = {
  COLOR_DAY: "#ff1493",
  COLOR_NIGHT: "#00f2ff",
} as const;

/**
 * UserMarkerObject
 *
 * Renderer-owned user marker object.
 * - owns beacon/zone lifetime
 * - binds itself to a live L4 anchor group
 * - applies anchor-local PVF3 offset
 *
 * NOTE:
 * This is intentionally React-free.
 * React should only drive target/theme updates from the control plane.
 */
export class UserMarkerObject {
  readonly root = new THREE.Group();

  readonly zone: THREE.InstancedMesh<THREE.BoxGeometry, THREE.MeshBasicNodeMaterial>;
  readonly beacon: THREE.Mesh<THREE.OctahedronGeometry, THREE.MeshStandardMaterial>;
  readonly light: THREE.PointLight;

  private registry: AnchorRegistry;
  private anchor_idx: bigint | null = null;

  private world_pos = new THREE.Vector3();
  private temp_obj = new THREE.Object3D();
  private scratch_v: V3 = [0, 0, 0];

  private center_pos: V3 = [0, 0, 0];

  constructor(registry: AnchorRegistry) {
    this.registry = registry;

    this.zone = new THREE.InstancedMesh(
      ZONE_SAMPLE_BOX,
      create_zone_material(),
      ZONE_SAMPLES.length,
    );
    this.zone.frustumCulled = false;
    this.zone.renderOrder = 5;

    const beacon_geom = new THREE.OctahedronGeometry(0.6, 0);
    const beacon_mat = new THREE.MeshStandardMaterial({
      color: beacon_style.COLOR_DAY,
      emissive: beacon_style.COLOR_DAY,
      emissiveIntensity: 0.5,
      roughness: 0.2,
      metalness: 0.8,
      flatShading: true,
    });

    this.beacon = new THREE.Mesh(beacon_geom, beacon_mat);
    this.beacon.position.set(...center3n([0, 0, BEACON_Z_BIAS], 1));

    this.light = new THREE.PointLight(
      beacon_style.COLOR_DAY,
      3,
      ZONE_RADIUS_CELL * U_PER_CELL,
    );

    this.root.visible = false;
    this.root.add(this.zone);
    this.root.add(this.beacon);
    this.root.add(this.light);
  }

  set_theme(is_night: boolean): void {
    const beacon_color = is_night ? beacon_style.COLOR_NIGHT : beacon_style.COLOR_DAY;
    const beacon_intensity = is_night ? 2 : 0.5;
    const light_intensity = is_night ? 10 : 3;

    this.beacon.material.color.set(beacon_color);
    this.beacon.material.emissive.set(beacon_color);
    this.beacon.material.emissiveIntensity = beacon_intensity;

    this.light.color.set(beacon_color);
    this.light.intensity = light_intensity;
  }

  set_coord(coord: PVF3 | null): void {
    if (coord === null) {
      this._detach_anchor();
      this.root.visible = false;
      return;
    }

    const pos: V3 = projection.unproject(fill3(0), coord.i, coord.f);
    pos[2] = get_ele(coord.i, coord.f) ?? 0;
    const center_pos: V3 = floor3n(pos, U_PER_CELL);

    this.center_pos[0] = center_pos[0];
    this.center_pos[1] = center_pos[1];
    this.center_pos[2] = center_pos[2];

    const center_coord: PVF3 = projection.project({ i: 0n, f: 0n }, ...center_pos);

    const next_anchor_idx = L4_IDX(center_coord.i);
    if (this.anchor_idx !== next_anchor_idx) {
      this._rebind_anchor(next_anchor_idx);
    }

    const anchor = L4_BASE(center_coord.i);
    const local = { i: ivec.sub(center_coord.i, anchor), f: center_coord.f };

    this.root.position.set(
      pvf3.to_real(pvf3.X(local)),
      pvf3.to_real(pvf3.Y(local)),
      pvf3.to_real(pvf3.Z(local)),
    );
    this.root.updateMatrix();

    this._rebuild_zone();
    this.root.visible = true;
  }

  tick(camera: THREE.PerspectiveCamera, elapsed: f64): void {
    this.beacon.getWorldPosition(this.world_pos);

    const dist: f64 = camera.position.distanceTo(this.world_pos);
    const visible: boolean = dist < FOG_FAR;

    this.root.visible = visible;
    if (!visible) return;

    this.beacon.rotation.z = elapsed * 1.2;
    this.beacon.position.z = BEACON_Z_BIAS + Math.sin(elapsed * 2) * 0.2;
  }

  dispose(): void {
    this._detach_anchor();
    this.root.clear();

    this.zone.geometry.dispose();
    this.zone.material.dispose();

    this.beacon.geometry.dispose();
    this.beacon.material.dispose();
  }

  private _rebuild_zone(): void {
    const mesh = this.zone;
    const temp_obj = this.temp_obj;
    const center_pos = this.center_pos;
    const v = this.scratch_v;

    mesh.count = ZONE_SAMPLES.length;

    const a_borders = mesh.geometry.attributes.a_borders as THREE.InstancedBufferAttribute;

    for (let i: u32 = 0; i < ZONE_SAMPLES.length; i++) {
      const { off, borders } = ZONE_SAMPLES[i];

      add3(center_pos, off, v);
      const coord: PVF3 = projection.project({ i: 0n, f: 0n }, ...v);
      const delta_ele: f64 = (get_ele(coord.i, coord.f) ?? 0) - center_pos[2];

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
  }

  private _rebind_anchor(next_anchor_idx: bigint): void {
    if (this.anchor_idx === next_anchor_idx) return;

    this._detach_anchor();

    const group = this.registry.link(next_anchor_idx);
    group.add(this.root);

    this.anchor_idx = next_anchor_idx;
  }

  private _detach_anchor(): void {
    if (this.anchor_idx === null) return;

    this.root.removeFromParent();
    this.registry.unlink(this.anchor_idx);
    this.anchor_idx = null;
  }
}
