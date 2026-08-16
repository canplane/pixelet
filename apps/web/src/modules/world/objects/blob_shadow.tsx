// @pxl/web/modules/world/objects/blob_shadow.tsx

/**
 * Cheap grounding cue for placed objects: one soft radial-gradient quad per
 * solid object, alpha-blended flat against the ground. No lighting, no
 * depth/normal sampling, no post pass — just an InstancedMesh of unit
 * planes, translated/scaled per object. See render/settings_store.ts's
 * show_blob_shadow doc comment for why this exists (a real shadow-map
 * pipeline was tried and removed for mobile-perf reasons; N8AO's own
 * per-pixel screen-space cost covers general contact/crevice occlusion but
 * is expensive — this covers the single highest-value case, "does a placed
 * object look grounded," for close to free).
 */

import { useLayoutEffect, useMemo, useRef } from "react";
import { invalidate } from "@react-three/fiber";
import * as THREE from "three";

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { get_object_entries_in_l2_column, l2_column_key_of, type PlacedObject, useObjectL2ColumnRevision } from "./store.ts";
import { chunk_origin_of, object_local_xyz } from "@/render/layer/objects/adapter.ts";

/** Shadow footprint relative to the object's own size — wider than the object itself reads as "resting on the ground," not just a matching silhouette. */
const SHADOW_SCALE: f64 = 1.4;
/** Sits just above the ground to avoid z-fighting with the terrain/object boxes it's meant to shadow. */
const Z_EPSILON: f64 = 0.02;
const TEX_SZ: u32 = 64;

/** Soft black-to-transparent radial gradient, baked once — the whole point of this technique is that it costs nothing per frame. */
function create_shadow_texture(): THREE.CanvasTexture {
  const canvas = document.createElement("canvas");
  canvas.width = TEX_SZ;
  canvas.height = TEX_SZ;
  const ctx = canvas.getContext("2d")!;
  const r = TEX_SZ / 2;
  const gradient = ctx.createRadialGradient(r, r, 0, r, r, r);
  gradient.addColorStop(0, "rgba(0,0,0,0.55)");
  gradient.addColorStop(0.7, "rgba(0,0,0,0.35)");
  gradient.addColorStop(1, "rgba(0,0,0,0)");
  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, TEX_SZ, TEX_SZ);
  const tex = new THREE.CanvasTexture(canvas);
  tex.needsUpdate = true;
  return tex;
}

let shadow_texture: THREE.CanvasTexture | null = null;
let shadow_material: THREE.MeshBasicMaterial | null = null;
const shadow_geometry = new THREE.PlaneGeometry(1, 1);

/** Lazily created — avoids touching document/canvas at module load (SSR-hostile, and this module is imported well before any Canvas mounts). */
function get_shadow_material(): THREE.MeshBasicMaterial {
  if (!shadow_material) {
    shadow_texture = create_shadow_texture();
    shadow_material = new THREE.MeshBasicMaterial({
      map: shadow_texture,
      transparent: true,
      depthWrite: false,
      toneMapped: false,
    });
  }
  return shadow_material;
}

const $matrix = new THREE.Matrix4();

type ObjectShadowsProps = {
  page_idx: IVec;
  page_off: IVec;
};

/** One L2 column's worth of blob-shadow instances — mirrors chunk_stage.tsx's own per-column mounting so shadows enter/exit the scene exactly when their column does, but independently: no shared state with the LOD-scheduled instanced terrain/object meshes it sits alongside. */
export function ObjectShadows(props: ObjectShadowsProps) {
  const { page_idx, page_off } = props;

  const object_key = useMemo(() => l2_column_key_of(page_idx, page_off), [page_idx, page_off]);
  const revision = useObjectL2ColumnRevision(object_key);
  const chunk_origin = useMemo(() => chunk_origin_of(page_idx, page_off), [page_idx, page_off]);

  const solids = useMemo(() => {
    return get_object_entries_in_l2_column(object_key)
      .filter((entry): entry is PlacedObject => entry.kind === "solid");
    // deliberately keyed on revision, not read directly — get_object_entries_in_l2_column
    // is a live, non-reactive read (same convention as every other WorldLayer consumer of
    // objects/store.ts); revision is what actually tells React this needs to re-run.
  }, [object_key, revision]);

  const mesh_ref = useRef<THREE.InstancedMesh>(null);

  useLayoutEffect(() => {
    const mesh = mesh_ref.current;
    if (!mesh) return;
    for (let i = 0; i < solids.length; i++) {
      // object_local_xyz returns the object's own base CORNER (same
      // convention objects/adapter.ts's cx0/cy0 use), not its center —
      // v3.ts's own center3(base, span) is what adds the +size/2 that
      // turns a corner into a visual center; this needs the same offset on
      // X/Y or the shadow centers on the wrong corner instead of the
      // object's actual footprint. Z is left alone: the corner's own Z is
      // already the object's ground-touching base, which is what the
      // shadow should sit at.
      const [cx, cy, z] = object_local_xyz(solids[i], chunk_origin);
      const half = solids[i].size / 2;
      const x = cx + half;
      const y = cy + half;
      const span = solids[i].size * SHADOW_SCALE;
      $matrix.makeScale(span, span, 1);
      $matrix.setPosition(x, y, z + Z_EPSILON);
      mesh.setMatrixAt(i, $matrix);
    }
    mesh.count = solids.length;
    mesh.instanceMatrix.needsUpdate = true;
    // frameloop="demand" (scene.tsx): mutating the instance buffer directly
    // like this doesn't itself schedule a repaint — same gotcha
    // scene.tsx's DevRenderToggleInvalidator exists to paper over for the
    // post-processing toggles, just here for an imperative buffer write
    // instead of a prop change.
    invalidate();
  }, [solids, chunk_origin]);

  if (solids.length === 0) return null;

  return (
    <instancedMesh
      key={solids.length}
      ref={mesh_ref}
      args={[shadow_geometry, get_shadow_material(), solids.length]}
      frustumCulled={false}
    />
  );
}
