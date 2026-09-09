// @pxl/web/modules/world/characters/companion.tsx

/**
 * A companion (person + pet) that sits at map_center, facing the camera
 * until it has somewhere to walk to. Whenever map_center stops changing —
 * after a drag, a click-to-navigate tween, a GPS recenter, whatever moved
 * it — it makes a fixed-duration catch-up walk there and ends up facing its
 * direction of travel, which it then just keeps (no snapping back to face
 * the camera once arrived). While map_center is still actively changing it
 * stays put. Ground height (Z) is re-sampled every frame regardless of
 * walking state — it "aligns", it doesn't walk there.
 *
 * Follow mode (is_follow_mode) is a different regime: map_center is a live
 * ~1 Hz GPS stream, so the settle-then-dash above never gets to rest and its
 * fixed dash curve fights the world-scroll damp. There the walker instead
 * glides toward the target every frame at the world-scroll rate (the two
 * cancel, holding it at screen center) and keeps its back to the camera by
 * counter-rotating azimuth — a nav-app "you" marker, map spinning under it.
 * See FOLLOW_LERP_K below and use_follow_mode.ts.
 *
 * Position/facing math lives in path.ts (no `three` import at all, so it
 * stays valid if this renderer is ever replaced); this file is the thin
 * Three.js-specific glue that reads that math and writes it into transforms.
 * Model loading/texturing/scaling is shared with the settings variant
 * picker's preview via use_character_model.ts.
 *
 * Assets: Kenney's "Mini Characters" (person) and "Cube Pets" (pet) packs,
 * both CC0, both real glTF. Their flat toy-block color palette doesn't match
 * the game's cozy pixel-art palette yet — swapping in a repainted or fully
 * custom skin is tracked separately, not blocking on it.
 */

import { memo, type MutableRefObject, useEffect, useLayoutEffect, useRef } from "react";
import * as THREE from "three";
import { useFrame } from "@react-three/fiber";

import type { f64 } from "@xpute/core/kit/word.ts";
import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { get_surface_z_at_local } from "../surface_height.ts";
import { heading_to, init_settle_tracker, sample_walk_plan, type SettleTracker, update_settle_tracker, type Vec2, type WalkPlan } from "./path.ts";
import { useCharacterModel } from "./use_character_model.ts";
import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { rim_cfg } from "@/modules/world/environment/config.ts";
import { set_rim_intensity } from "@/modules/world/render/lit_gltf_material.ts";
import { useCameraStore } from "@/modules/camera/store.ts";
import { camera_cfg } from "@/modules/camera/config.ts";
import { damp_alpha } from "@/modules/camera/orbit.ts";
import { useCompanionLoadStore } from "./store.ts";
import { ChatBubble } from "./chat_bubble.tsx";
import { type CharacterVariant, find_person_variant, find_pet_variant } from "@pxl/shared/assets/characters/variants.ts";

// A little to the side of the person, so the two don't occupy the same
// footprint — arbitrary companion-following offset, not derived from
// anything about either asset.
const PET_OFFSET: Vec2 = { x: -0.6, y: -0.6 };

// Catch-up always takes this long, whatever the distance — see
// sample_walk_plan's own doc comment for why a fixed duration beats a fixed
// speed here.
const CATCHUP_DURATION_S: f64 = 0.6;

// How long map_center has to sit still before we call it "settled" and plan
// a catch-up — see update_settle_tracker's own doc comment for why this
// reacts to the target itself rather than to any one specific gesture.
const SETTLE_DELAY_S: f64 = 0.15;

// Follow mode only: the settle-then-dash catch-up above is built for
// occasional map_center jumps (a pan, a click-to-navigate, a recenter) with
// long still gaps between. Follow mode feeds it a ~1 Hz stream of GPS jumps
// instead, so it never gets to just "be there" — it sits perpetually
// mid-dash, and that dash (a fixed 0.6 s curve after a 0.15 s hold) fights
// use_frame_translation's world-scroll damp, swaying the character along the
// travel axis on every fix. So in follow mode the character skips all of
// that and glides toward the target with a plain per-frame exponential lerp
// at the SAME rate the world scroll damps (camera_cfg POS_DAMP). Because the
// two run at the same rate toward the same point, they cancel: the
// character's world position stays at screen center, with only its own
// (single) catch-up residual showing as a small lean. use_follow_mode.ts
// drops its own position EMA to match, so this lerp is the one and only
// position-smoothing stage.
const FOLLOW_LERP_K: f64 = camera_cfg.smooth.POS_DAMP;

// Follow mode: the walker's rotation.z, set every frame as
// `FOLLOW_FACING - <parent's world Z rotation>` so its *world* facing stays
// constant while the map spins under it (nav-app "you" arrow). The idle
// "face the camera" branch below works out to a model whose forward is -Y in
// world space at rotation.z 0 (the camera sits at view -Y); +PI turns that
// to +Y — into the screen, back to the camera.
const FOLLOW_FACING: f64 = Math.PI;

// Follow-mode walk clip: play it while the glide has a real gap to close,
// then hold it that much longer before dropping to idle. The glide catches
// up between GPS fixes (gap -> ~0, then the next fix jumps it again), so
// without the linger the clip would strobe on/off once per fix; a linger a
// bit longer than a fix interval keeps it steady while actually moving. EPS
// is well under one fix's worth of travel (~0.5u at a walking pace) so a
// real step trips it, over the near-zero settled gap so standing still
// doesn't.
const FOLLOW_WALK_EPS: f64 = 0.03;
const FOLLOW_WALK_LINGER_S: f64 = 1.5;

const IDLE_CLIP = "idle";

// Scratch for reading the parent frame's current world rotation (follow-mode
// facing) — reused every frame, never held across the await-free useFrame
// body. Two Walker instances share these, but their useFrame callbacks run
// sequentially on one thread and consume the value before yielding.
const $parent_quat = new THREE.Quaternion();
const $parent_euler = new THREE.Euler();

type WalkerProps = {
  /** Mutated in place by Companion every render, never replaced — Walker
   * reads .current fresh each frame instead of taking these as reactive
   * props, specifically so map_center's every-frame updates (it tracks the
   * live camera target) don't force Walker itself to re-render 60x/sec for
   * no reason. */
  origin_ref: MutableRefObject<IVec>;
  /** Full floating-origin IVec (not just its XY page index) — needed to
   * convert a local XY probe into the absolute PVF3 get_surface_z_at_local
   * queries placed objects with. Changes far less often than target_ref
   * (only on an L4 rebase), but threaded the same ref way for consistency
   * and to avoid forcing a re-render on every map_center tick. */
  map_origin_ref: MutableRefObject<IVec>;
  target_ref: MutableRefObject<Vec2>;
  /** map_center's own current Z — the hint get_surface_z_at_local uses to
   * pick which shelf to land on when there's a gap under this walker's
   * XY. Not this walker's own target height (the pet's target_ref has an
   * XY offset from map_center, but there's only one map_center Z). */
  target_z_ref: MutableRefObject<f64>;
  variant: CharacterVariant;
  /** Only the person talks — the pet doesn't get a chat bubble. */
  show_chat_bubble?: boolean;
  /** Which half of useCompanionLoadStore this instance reports readiness
   * into — see store.ts's own doc comment on why that state lives outside
   * this component's tree. */
  role: "person" | "pet";
};

const Walker = memo(function Walker(props: WalkerProps) {
  const { origin_ref, map_origin_ref, target_ref, target_z_ref, variant, show_chat_bubble, role } = props;
  const { root: character_root, material: character_material, actions } = useCharacterModel(variant.model_url, variant.skin_url, variant.height_u);

  // Same live show_rim_light wiring decor/asset_store.ts does for decor's
  // materials — use_character_model.ts
  // itself doesn't know this setting (matching instanced_model.ts's own
  // reason for the same split), so whichever real render path needs it live
  // owns the subscription instead.
  const show_rim_light = useSettingsStore((state) => state.show_rim_light);
  useEffect(() => {
    if (character_material) set_rim_intensity(character_material, show_rim_light ? rim_cfg.INTENSITY : 0);
  }, [character_material, show_rim_light]);

  // Rare toggle, so subscribing (and the re-render it costs) is fine — unlike
  // map_center, this doesn't churn per frame.
  const is_follow_mode = useCameraStore((state) => state.is_follow_mode);
  const was_follow_mode_ref = useRef(false);

  useEffect(() => {
    const set_ready = role === "person" ? useCompanionLoadStore.getState().set_person_ready : useCompanionLoadStore.getState().set_pet_ready;
    set_ready(character_root !== null);
  }, [character_root, role]);

  const walk_plan_ref = useRef<WalkPlan | null>(null);
  const settle_ref = useRef<SettleTracker>(init_settle_tracker(target_ref.current, 0));
  // Faces the camera exactly once — the moment it first has a real parent
  // transform to resolve the camera's position against — and never again
  // after that, not even while still idle before the first catch-up walk.
  // Rotating the camera (azimuth) around a companion that's just standing
  // there is not a reason for it to keep turning to track the viewer; only
  // its very first pose should ever be camera-relative.
  const initial_facing_done_ref = useRef(false);
  const current_clip_ref = useRef<string | null>(null);
  const cam_world_pos = useRef(new THREE.Vector3()).current;

  // One-time placement at the *current* target, once the model exists. Not
  // via useFrame: target_ref.current can already be thousands of units away
  // from the group's default (0,0,0) — the coordinate system's own origin,
  // not "near the camera" — since it's map_center's offset within its
  // enclosing L4 tile. Walking there at any sane pace would take far too
  // long; only *subsequent* moves should animate.
  const placed_for_ref = useRef<THREE.Group | null>(null);
  useLayoutEffect(() => {
    if (!character_root || placed_for_ref.current === character_root) return;
    character_root.position.set(target_ref.current.x, target_ref.current.y, 0);
    placed_for_ref.current = character_root;
    // target_ref deliberately not read reactively here: this is a one-time
    // initial placement, not a live binding — subsequent moves go through
    // the settle-tracked catch-up walk in useFrame below.
  }, [character_root]);

  // Tracks the last map_origin this loop saw, to snap (not glide) across an
  // L4 rebase — target_ref jumps ~4096u then, and use_frame_translation snaps
  // the world scroll for it.
  const prev_map_origin_ref = useRef<IVec>(map_origin_ref.current);
  // Clock time until which the follow-mode walk clip stays on — see
  // FOLLOW_WALK_LINGER_S.
  const follow_walk_until_ref = useRef<f64>(0);

  useFrame((state, delta) => {
    if (!character_root) return;

    const now: f64 = state.clock.getElapsedTime();
    const origin_page_idx_xy = origin_ref.current;
    const target = target_ref.current;
    const rebased: boolean = map_origin_ref.current !== prev_map_origin_ref.current;

    // px/py: this frame's XY for the walker. is_walking: whether the move
    // clip should play. Filled in by whichever branch runs.
    let px: f64;
    let py: f64;
    let is_walking: boolean;

    if (is_follow_mode) {
      // Glide toward the target at the world-scroll damp rate — no settle,
      // no dash. The two motions cancel, holding the walker at screen center
      // (see FOLLOW_LERP_K's doc comment). rotation is driven every frame
      // below, so the settle/one-time-facing bookkeeping stays untouched
      // and resumes cleanly on exit. Snap (not glide) on the entry frame and
      // across an L4 rebase: both are one-shot jumps use_frame_translation
      // itself snaps through, and gliding a browse-distance gap would fly the
      // walker in from off-screen.
      if (rebased || !was_follow_mode_ref.current) {
        px = target.x;
        py = target.y;
      } else {
        const a: f64 = damp_alpha(FOLLOW_LERP_K, delta);
        px = character_root.position.x + (target.x - character_root.position.x) * a;
        py = character_root.position.y + (target.y - character_root.position.y) * a;
      }

      // Back to the camera, held there while the map spins underneath —
      // counter-rotate whatever azimuth the parent frame currently carries
      // so the walker's final world facing stays FOLLOW_FACING.
      if (character_root.parent) {
        character_root.parent.getWorldQuaternion($parent_quat);
        $parent_euler.setFromQuaternion($parent_quat, "ZYX");
        character_root.rotation.z = FOLLOW_FACING - $parent_euler.z;
      }

      if (Math.hypot(target.x - px, target.y - py) > FOLLOW_WALK_EPS) {
        follow_walk_until_ref.current = now + FOLLOW_WALK_LINGER_S;
      }
      is_walking = now < follow_walk_until_ref.current;
      walk_plan_ref.current = null;
    } else {
      // On the frame follow mode turns off, rebase the settle tracker to the
      // live target so update_settle_tracker doesn't read the (large)
      // accumulated while-following delta as one giant just-settled dash.
      if (was_follow_mode_ref.current) {
        settle_ref.current = init_settle_tracker(target, now);
        walk_plan_ref.current = null;
      }

      // map_center settling (regardless of what moved it — drag, a click-to
      // -navigate tween, GPS recenter, ...) plans a fixed-duration catch-up
      // from wherever the walker actually is right now. While it's still
      // actively changing, cancel any in-progress catch-up and hold position
      // — see update_settle_tracker's own doc comment for why this reacts to
      // the target itself rather than to one specific gesture's own status.
      const { tracker, just_settled, moved } = update_settle_tracker(settle_ref.current, target, now, SETTLE_DELAY_S);
      settle_ref.current = tracker;
      if (just_settled) {
        walk_plan_ref.current = {
          start: { x: character_root.position.x, y: character_root.position.y },
          target: tracker.settled,
          start_time: now,
          duration: CATCHUP_DURATION_S,
        };
      }
      if (moved) {
        walk_plan_ref.current = null;
      }

      const plan = walk_plan_ref.current;
      const step = plan ? sample_walk_plan(plan, now) : { x: character_root.position.x, y: character_root.position.y, heading: null, arrived: true };
      if (step.arrived) walk_plan_ref.current = null;

      px = step.x;
      py = step.y;
      is_walking = step.heading !== null;

      // Facing: direction of travel while walking. Otherwise, a one-time
      // camera-facing pose the very first frame a parent transform exists to
      // resolve it against — never recomputed again afterward (not on later
      // idle frames, not as azimuth changes), and never once a walk has
      // happened, at which point arriving just holds whatever heading that
      // walk left it on (rotation.z is simply left untouched in that case).
      // use_character_model's up-axis wrapping maps the model's native
      // forward to +Y in this group's frame, hence the +PI/2 offset from a
      // raw heading to the actual object rotation.
      if (is_walking) {
        character_root.rotation.z = step.heading! + Math.PI / 2;
      } else if (!initial_facing_done_ref.current && character_root.parent) {
        state.camera.getWorldPosition(cam_world_pos);
        character_root.parent.worldToLocal(cam_world_pos);
        const heading = heading_to({ x: step.x, y: step.y }, { x: cam_world_pos.x, y: cam_world_pos.y });
        character_root.rotation.z = heading + Math.PI / 2;
        initial_facing_done_ref.current = true;
      }
    }

    prev_map_origin_ref.current = map_origin_ref.current;
    was_follow_mode_ref.current = is_follow_mode;

    // Z always aligns to the surface under the walker's (possibly just-
    // updated) x/y, independent of whether it moved this frame at all —
    // terrain, or the top of a placed object standing on it, whichever is
    // higher (get_surface_z_at_local's own doc comment).
    const surface_z: f64 | null = get_surface_z_at_local(origin_page_idx_xy, map_origin_ref.current, px, py, target_z_ref.current);
    character_root.position.set(px, py, surface_z ?? character_root.position.z);

    // Crossfade only on an actual state change, not every frame.
    const desired_clip = is_walking ? variant.move_clip : IDLE_CLIP;
    if (current_clip_ref.current !== desired_clip) {
      actions[current_clip_ref.current ?? ""]?.fadeOut(0.2);
      actions[desired_clip]?.reset().fadeIn(0.2).play();
      current_clip_ref.current = desired_clip;
    }

    // frameloop="demand" (see scene.tsx / LocationPuck's own use of this):
    // nothing else invalidates on its own while this loop and the clip's
    // internal mixer keep animating, so this component must ask for the
    // next frame itself for as long as it's mounted.
    state.invalidate();
  });

  return character_root
    ? (
      <primitive object={character_root}>
        {show_chat_bubble && <ChatBubble anchor_z={variant.height_u} />}
      </primitive>
    )
    : null;
});

type CompanionProps = {
  origin_page_idx_xy: IVec;
  map_origin: IVec;
  center_x: f64;
  center_y: f64;
  center_z: f64;
};

export function Companion(props: CompanionProps) {
  const { origin_page_idx_xy, map_origin, center_x, center_y, center_z } = props;

  const person_variant_key = useSettingsStore((state) => state.person_variant_key);
  const pet_variant_key = useSettingsStore((state) => state.pet_variant_key);
  const person_variant = find_person_variant(person_variant_key);
  const pet_variant = find_pet_variant(pet_variant_key);

  // Stable containers, mutated in place every render — see WalkerProps's
  // own doc comment for why these are refs rather than plain props.
  const origin_ref = useRef<IVec>(origin_page_idx_xy);
  origin_ref.current = origin_page_idx_xy;
  const map_origin_ref = useRef<IVec>(map_origin);
  map_origin_ref.current = map_origin;
  const target_z_ref = useRef<f64>(center_z);
  target_z_ref.current = center_z;
  const person_target_ref = useRef<Vec2>({ x: center_x, y: center_y });
  person_target_ref.current.x = center_x;
  person_target_ref.current.y = center_y;
  const pet_target_ref = useRef<Vec2>({ x: center_x + PET_OFFSET.x, y: center_y + PET_OFFSET.y });
  pet_target_ref.current.x = center_x + PET_OFFSET.x;
  pet_target_ref.current.y = center_y + PET_OFFSET.y;

  return (
    <>
      <Walker
        key={person_variant.key}
        origin_ref={origin_ref}
        map_origin_ref={map_origin_ref}
        target_ref={person_target_ref}
        target_z_ref={target_z_ref}
        variant={person_variant}
        show_chat_bubble
        role="person"
      />
      <Walker
        key={pet_variant.key}
        origin_ref={origin_ref}
        map_origin_ref={map_origin_ref}
        target_ref={pet_target_ref}
        target_z_ref={target_z_ref}
        variant={pet_variant}
        role="pet"
      />
    </>
  );
}
