// @pxl/web/modules/world/terrain/chunk.tsx

import { memo, useEffect, useLayoutEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { useFrame } from "@react-three/fiber";

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L2_CENTER } from "@pxl/shared/spec/space/indexer.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";

import { GROUND_CH_KEYS, type GroundChannelKey, type Terrain } from "@pxl/shared/bake/format.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import { get_instance_cnt, get_lod_guide, type LODStep } from "@/render/lod.ts";
import { decode_ground_rgba } from "@/render/terrain/shader_meta.ts";
import { shadow_cfg } from "@/modules/world/render/config.ts";
import { useDebugStore } from "@/app/dev/debug_store.ts";

import { create_ground_color_texture, create_heightmap, create_splatmap } from "./shader/tex.ts";
import { create_mat } from "./shader/mat.ts";

const ROW_STRIDE: u32 = CHUNK_CELL_SPAN;
const CHUNK_CELL_SPAN2: u32 = CHUNK_CELL_SPAN * CHUNK_CELL_SPAN;

const CHUNK_BOX = new THREE.BoxGeometry(1, 1, 1);

const GROUND_Z_BIAS: f64 = -0.01;

/** Seconds between LOD distance re-checks (per chunk). */
const LOD_RECHECK_INTERVAL: f64 = 0.15;

// Chunks whose center is farther than this from the local (floating-origin)
// scene origin cannot possibly intersect the sun/moon shadow camera's ortho
// frustum (a fixed +-FRUSTUM_HALF_EXTENT box centered there, see
// environment.tsx) — their own castShadow gets turned off in the LOD
// recheck below instead of relying on frustumCulled, which chunk meshes
// keep permanently disabled (see the instancedMesh JSX below for why: their
// position is GPU-computed per-instance in the vertex shader, not via
// instanceMatrix, so three's own auto bounding-sphere is wrong and
// frustumCulled=false was the existing workaround — but that flag also
// silently skips the shadow camera's own frustum test, so every cached
// chunk was being submitted to the shadow pass every frame regardless of
// distance). Margin is the chunk footprint's own half-diagonal (the
// farthest a chunk's own corner can sit from its own center) — purely
// horizontal, same as the shadow frustum's own left/right/top/bottom bounds
// and shader.ts's shadow_dist fade, so the skirt's much deeper vertical
// drop (shader.ts's position_core, up to -200u at chunk edges) never
// factors in here.
const CHUNK_HALF_DIAGONAL: f64 = CHUNK_CELL_SPAN * U_PER_CELL * Math.SQRT1_2;
const SHADOW_CULL_DIST: f64 = shadow_cfg.FRUSTUM_HALF_EXTENT + CHUNK_HALF_DIAGONAL;

// page_off components repeat identically across every page (see
// page_to_chunks in render/terrain/chunk.ts — always a handful of small
// fixed offsets), so folding page_idx in at a scale bigger than page_off's
// own max keeps two different pages' same-offset chunks from ever landing on
// the same hash — without this, e.g. every page's (0,0) chunk would blend-
// transition in lockstep, a visible grid-aligned artifact of its own.
const PAGE_OFF_HASH_SCALE = 128;

/** Deterministic [0,1) hash — same sin/fract trick shader.ts's own GLSL hash
 * functions use, evaluated once here on the CPU (at chunk creation) instead
 * of per-fragment on the GPU. Feeds u_blend_r (see position_core's step 0 in
 * shader.ts) — chunk.tsx computes this once and it never changes prescribed
 * for a chunk's whole lifetime, unlike the per-frame LOD/blend uniforms. */
function hash01(x: f64, y: f64): f64 {
  const s = Math.sin(x * 12.9898 + y * 78.233) * 43758.5453123;
  return s - Math.floor(s);
}

type TerrainChunkProps = {
  /** Owning page's index — only used to seed this chunk's u_blend_r (see
   * hash01 above); every other position/anchoring concern is handled by
   * page_off relative to the page's own AnchorNode (layer.tsx), same as before. */
  page_idx: IVec;
  /** Page-relative local chunk offset. */
  page_off: IVec;
  chunk: Terrain;
};
export const TerrainChunk = memo((props: TerrainChunkProps) => {
  const { page_idx, page_off, chunk } = props;
  const { ground, ele } = chunk;

  // per-chunk constant, not per-frame — see hash01's own comment.
  const blend_r = useMemo(
    () =>
      hash01(
        ivec.to_real(ivec.X(page_idx)) * PAGE_OFF_HASH_SCALE + ivec.to_real(ivec.X(page_off)),
        ivec.to_real(ivec.Y(page_idx)) * PAGE_OFF_HASH_SCALE + ivec.to_real(ivec.Y(page_off)),
      ),
    [page_idx, page_off],
  );

  const mesh_ref = useRef<THREE.InstancedMesh>(null);
  // second draw for the LOD-boundary blend (see lod.ts's get_lod_guide and
  // shader.ts's position_core) — always mounted, but left at count=0 outside
  // a blend band so it costs nothing when this chunk isn't transitioning.
  // kept as a permanent second mesh rather than conditionally mounted:
  // get_lod_guide's blend_active flips in and out multiple times a second
  // right at a band's edge, and mounting/unmounting a whole instancedMesh +
  // material on every flip (React commit, GPU buffer alloc/dispose) would
  // cost far more than one always-present zero-count draw does.
  const blend_mesh_ref = useRef<THREE.InstancedMesh>(null);

  const splatmaps = useMemo(() => {
    return Object.fromEntries(
      GROUND_CH_KEYS.map((key) => [key, create_splatmap(ground[key], ROW_STRIDE)]),
    ) as Record<GroundChannelKey, THREE.DataTexture>;
  }, [ground]);

  // dev_panel.tsx's "건물" toggle — read up here (not just as a live-uniform
  // sync below) because ground_color's own bake needs to know about it too:
  // decode_ground_rgba only folds STRUCTURE into the blend when this is on
  // (see its own doc comment), so toggling it has to re-bake, not just
  // flip a uniform, for low-spec mode's colors to agree with what the
  // is_structure-gated uniform sync below does for every other effect.
  const show_structures = useDebugStore((state) => state.show_structures);

  // real per-chunk mip-blended color bake — see shader_meta.ts's
  // decode_ground_rgba and mat.ts's own u_low_spec comment for why this
  // exists alongside (not replaced by) the shared ground_layers texture
  // array mat.ts also builds.
  const ground_color = useMemo(
    () => create_ground_color_texture(decode_ground_rgba(ground, ROW_STRIDE, show_structures), ROW_STRIDE),
    [ground, show_structures],
  );
  const heightmap = useMemo(() => create_heightmap(ele, ROW_STRIDE), [ele]);
  const { mat, depth_mat } = useMemo(
    () => create_mat(splatmaps, heightmap, ground_color, ROW_STRIDE, U_PER_CELL, blend_r),
    [splatmaps, heightmap, ground_color, blend_r],
  );
  // independent material instance (not the same object as mat/depth_mat) —
  // it needs its own u_lod_step/u_blend_role, which differ from the
  // primary's whenever a blend is active. Cheap to keep around otherwise:
  // same source textures, no extra GPU memory beyond the small uniform
  // buffers and a second (usually 0-instance) draw call. Shares the exact
  // same blend_r as the primary above (same chunk, same hash input) — the
  // two draws MUST agree on this value, or they'd independently decide
  // "I'm the visible one" using different thresholds and could both (or
  // neither) claim the same cell population.
  const { mat: blend_mat, depth_mat: blend_depth_mat } = useMemo(
    () => create_mat(splatmaps, heightmap, ground_color, ROW_STRIDE, U_PER_CELL, blend_r),
    [splatmaps, heightmap, ground_color, blend_r],
  );

  useEffect(() => {
    return () => {
      Object.values(splatmaps).forEach((tex) => tex.dispose());
      ground_color.dispose();
      heightmap.dispose();
      mat.dispose();
      depth_mat.dispose();
      blend_mat.dispose();
      blend_depth_mat.dispose();
    };
  }, [splatmaps, ground_color, heightmap, mat, depth_mat, blend_mat, blend_depth_mat]);

  // is_structure-gated effects (resolve_ground_layer in texture-pack mode,
  // material-grain/side-tint tweaks further down in the fragment shader) —
  // a live uniform write, not a useMemo dep on mat/blend_mat above (those
  // two are already the expensive-to-rebuild ShaderMaterial instances, and
  // ground_color's own useMemo above is what actually reacts to
  // show_structures for low-spec mode's colors specifically).
  // depth_mat/blend_depth_mat don't need their own write: mat.ts's
  // core_uniforms object is shared by reference between mat/depth_mat (and
  // separately between blend_mat/blend_depth_mat), the same trick
  // u_lod_step/u_blend_t already rely on there.
  useEffect(() => {
    mat.uniforms.u_show_structures.value = show_structures;
    blend_mat.uniforms.u_show_structures.value = show_structures;
  }, [show_structures, mat, blend_mat]);

  // dev_panel.tsx's "텍스처팩" toggle — same live-uniform-write pattern as
  // show_structures above, see debug_store.ts's own doc comment for what
  // this actually switches.
  const low_spec = useDebugStore((state) => state.low_spec);
  useEffect(() => {
    mat.uniforms.u_low_spec.value = low_spec;
    blend_mat.uniforms.u_low_spec.value = low_spec;
  }, [low_spec, mat, blend_mat]);

  const chunk_world_pos = useRef(new THREE.Vector3());

  // getWorldPosition() below resolves chunk_world_pos.z from this mesh's own
  // LOCAL z (GROUND_Z_BIAS, a small placeholder — see its own comment,
  // "terrain height is a GPU-only vertex-shader displacement") through the
  // floating-origin frame's translation (-map_center's real elevation, see
  // use_frame_translation.ts). That composition is already correct for
  // every chunk *except* the placeholder itself: swap GROUND_Z_BIAS for this
  // chunk's own real elevation (sampled once from its own ele data, the
  // chunk's dense elevation array already resident on the CPU — no new
  // fetch) and the same chain yields this chunk's true local-frame Z,
  // directly comparable against camera.position (which lives in that same
  // frame — see use_camera.ts).
  //
  // Full-chunk mean, not a single center-cell sample: a lone sample is
  // needlessly noisy at real terrain peaks/ridges specifically — two
  // adjacent chunks straddling a hillside can have wildly different center-
  // cell heights even though the real terrain between them is continuous,
  // which shifts each chunk's own LOD threshold-crossing point away from its
  // neighbor's by that same discontinuity, for no reason a real average
  // wouldn't already smooth out. (This alone turned out not to be the cause
  // of a real LOD-popping-near-a-peak bug reported around the same time —
  // that one traced back to a hysteresis/blend-band mismatch in lod.ts, see
  // calculate_lod_step's own doc comment — but it's still a real, free
  // improvement in its own right: a single sample is a strictly worse
  // elevation estimate than the mean of the whole chunk for essentially
  // zero extra cost.) One cheap pass over a typed array, done once at chunk
  // creation, not per frame.
  const chunk_z_correction = useMemo(() => {
    let sum = 0;
    for (let i = 0; i < ele.length; i++) sum += ele[i];
    const mean_ele_cell: f64 = sum / ele.length;
    const real_elevation_u: f64 = mean_ele_cell * U_PER_CELL;
    return real_elevation_u - GROUND_Z_BIAS;
  }, [ele]);

  useLayoutEffect(() => {
    if (!mesh_ref.current) return;
    mesh_ref.current.frustumCulled = false;
    mesh_ref.current.count = CHUNK_CELL_SPAN2;
    // R3F has no `customDepthMaterial` JSX prop — it's an imperative-only
    // three.js field WebGLShadowMap reads directly off the object, so it has
    // to be set by hand here rather than declared on <instancedMesh>.
    mesh_ref.current.customDepthMaterial = depth_mat;
  }, [depth_mat]);

  useLayoutEffect(() => {
    if (!blend_mesh_ref.current) return;
    blend_mesh_ref.current.frustumCulled = false;
    blend_mesh_ref.current.count = 0;
    blend_mesh_ref.current.customDepthMaterial = blend_depth_mat;
  }, [blend_depth_mat]);

  // ---- LOD ----

  const current_lod_step = useRef<LODStep>(1);
  const lod_recheck_elapsed = useRef<f64>(0);
  useFrame(({ camera }, delta) => {
    if (!mesh_ref.current || !blend_mesh_ref.current) return;

    // updateWorldMatrix + getWorldPosition walks this chunk's whole parent chain, and
    // with up to a few hundred chunks loaded at once that's a real per-frame cost.
    // LOD doesn't need per-frame precision, so only re-check a handful of times a
    // second instead of on every tick.
    lod_recheck_elapsed.current += delta;
    if (lod_recheck_elapsed.current < LOD_RECHECK_INTERVAL) return;
    lod_recheck_elapsed.current = 0;

    // R3F/three.js only refreshes matrixWorld once per tick, after all useFrame
    // callbacks run — reading it here would be one frame stale. That's usually
    // negligible, but right after a floating-origin rebase (an instant, large
    // position snap) it's a real error for that one frame, which can push the
    // LOD up and get stuck there due to the hysteresis below. Force a fresh
    // parent-chain update before sampling.
    mesh_ref.current.updateWorldMatrix(true, false);
    mesh_ref.current.getWorldPosition(chunk_world_pos.current);

    // origin (not camera) distance — see SHADOW_CULL_DIST's own comment.
    // Both meshes toggle together: the blend mesh is just this chunk's
    // second draw for a LOD-boundary dither, same position, so it can never
    // be shadow-eligible when the primary isn't (or vice versa).
    const shadow_eligible = Math.hypot(chunk_world_pos.current.x, chunk_world_pos.current.y) <= SHADOW_CULL_DIST;
    if (mesh_ref.current.castShadow !== shadow_eligible) {
      mesh_ref.current.castShadow = shadow_eligible;
      blend_mesh_ref.current.castShadow = shadow_eligible;
    }

    // Real 3D distance from the camera to this chunk's own actual elevation
    // (chunk_z_correction above turns chunk_world_pos.z's GROUND_Z_BIAS
    // placeholder into this chunk's real local-frame height) — for a fixed
    // FOV, on-screen apparent size is proportional to 1/distance, so this is
    // a cheap, standard stand-in for "how many pixels does this chunk cover"
    // (the same approximation most game engines' distance-based LOD systems
    // already use, screen-projected-corner math included, just without the
    // extra matrix work). Used to be X/Y only, which was *right* about not
    // wanting the chunk's own always-≈0 placeholder Z in the mix, but wrong
    // about it being impossible to fix cheaply — chunk_z_correction is that
    // fix, not a reason to drop Z entirely: real elevation differences (a
    // chunk on a hill vs. one in a valley) and real camera tilt/zoom height
    // both now count, same as any other axis, instead of being a deliberate
    // 2D-only carve-out.
    const dx = camera.position.x - chunk_world_pos.current.x;
    const dy = camera.position.y - chunk_world_pos.current.y;
    const dz = camera.position.z - (chunk_world_pos.current.z + chunk_z_correction);
    const dist = Math.hypot(dx, dy, dz);

    // derive the next LOD state from distance.
    const guide = get_lod_guide(dist, current_lod_step.current);

    // apply GPU-side changes only when the LOD step actually changes.
    if (guide.lod_step !== current_lod_step.current) {
      current_lod_step.current = guide.lod_step;
      mesh_ref.current.count = guide.instance_cnt;
      mat.uniforms.u_lod_step.value = guide.lod_step;
      depth_mat.uniforms.u_lod_step.value = guide.lod_step;
    }

    if (guide.blend_active && guide.blend_finer_step !== null && guide.blend_coarser_step !== null) {
      const blend_step: LODStep = guide.lod_step === guide.blend_finer_step ? guide.blend_coarser_step : guide.blend_finer_step;
      const primary_role: number = guide.lod_step === guide.blend_finer_step ? 0 : 1;

      mat.uniforms.u_blend_t.value = guide.coarse_fraction;
      mat.uniforms.u_blend_role.value = primary_role;
      depth_mat.uniforms.u_blend_t.value = guide.coarse_fraction;
      depth_mat.uniforms.u_blend_role.value = primary_role;

      if (blend_mesh_ref.current.count === 0 || blend_mat.uniforms.u_lod_step.value !== blend_step) {
        blend_mesh_ref.current.count = get_instance_cnt(blend_step);
        blend_mat.uniforms.u_lod_step.value = blend_step;
        blend_depth_mat.uniforms.u_lod_step.value = blend_step;
      }
      blend_mat.uniforms.u_blend_t.value = guide.coarse_fraction;
      blend_mat.uniforms.u_blend_role.value = 1 - primary_role;
      blend_depth_mat.uniforms.u_blend_t.value = guide.coarse_fraction;
      blend_depth_mat.uniforms.u_blend_role.value = 1 - primary_role;
    } else if (blend_mesh_ref.current.count !== 0) {
      // left the band — collapse back to a single normal draw. Resetting
      // u_blend_t/role on the primary isn't strictly required (role 0 + t=0
      // is the inert default it already had outside a band) but cheap
      // insurance against a stale value if this ever gets hit mid-band by a
      // sudden jump (see the hysteresis-mismatch guard in lod.ts).
      mat.uniforms.u_blend_t.value = 0;
      mat.uniforms.u_blend_role.value = 0;
      depth_mat.uniforms.u_blend_t.value = 0;
      depth_mat.uniforms.u_blend_role.value = 0;
      blend_mesh_ref.current.count = 0;
    }
  });

  const chunk_position: V3 = useMemo(() => {
    const chunk_center: IVec = L2_CENTER(page_off);
    return [
      ivec.to_real(ivec.X(chunk_center)),
      ivec.to_real(ivec.Y(chunk_center)),
      GROUND_Z_BIAS,
    ];
  }, [page_off]);
  return (
    <>
      <instancedMesh
        ref={mesh_ref}
        // the third arg is the maximum allocation count for the densest LOD.
        args={[CHUNK_BOX, undefined, CHUNK_CELL_SPAN2]}
        position={chunk_position}
        frustumCulled={false}
        // castShadow: this terrain *is* where "buildings" live (elevated
        // cells, no separate 3D building mesh exists yet) — without this,
        // nothing in real content casts a shadow at all. Backed by
        // customDepthMaterial above so the caster's silhouette matches the
        // heightmap-displaced shape instead of three's default (flat,
        // undisplaced) depth material. `true` here is only the mount-time
        // default — the LOD useFrame above takes over within one recheck
        // interval, flipping this off for chunks too far from the shadow
        // frustum to matter (see SHADOW_CULL_DIST).
        castShadow
        receiveShadow
      >
        <primitive object={mat} attach="material" />
      </instancedMesh>

      <instancedMesh
        ref={blend_mesh_ref}
        args={[CHUNK_BOX, undefined, CHUNK_CELL_SPAN2]}
        position={chunk_position}
        frustumCulled={false}
        castShadow
        receiveShadow
      >
        <primitive object={blend_mat} attach="material" />
      </instancedMesh>
    </>
  );
});
TerrainChunk.displayName = "TerrainChunk";
