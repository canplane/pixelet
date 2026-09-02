// @pxl/web/modules/world/characters/use_character_model.ts

import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { useAnimations } from "@react-three/drei";
import { SkeletonUtils } from "three-stdlib";

import type { f64 } from "@xpute/core/kit/word.ts";
import { load_gltf, load_texture } from "@/modules/world/gltf_cache.ts";

export type CharacterModel = {
  /** Ready-to-place group: textured, scaled to height_u, Y-up geometry
   * wrapped so the group's own local Z axis is up. Callers own its position
   * and rotation entirely — this hook never touches them after construction.
   * Null until the model has actually finished loading. */
  root: THREE.Group | null;
  actions: ReturnType<typeof useAnimations>["actions"];
};

/**
 * Loads a Kenney-style rigged glTF (model_url) sharing a flat-swatch
 * colormap texture (skin_url), textures it with an unlit material, scales
 * it to a real-world height, and fixes it up from glTF's Y-up convention to
 * this app's Z-up one. Shared by both the in-world companion (companion.tsx)
 * and the settings variant-picker preview (character_preview.tsx) — the
 * only difference between those two is what drives root.position/rotation
 * afterward, which is entirely their own concern.
 *
 * Deliberately NOT built on drei's useGLTF/useTexture (Suspense-integrated
 * loaders): in this app, a component calling those from inside a fast-
 * changing parent (companion.tsx's Walker re-renders on every map_center
 * tick) was observed re-suspending on effectively every frame instead of
 * resolving once and staying resolved — each "successful" resolution
 * produced a *new* Object3D (confirmed via per-load UUIDs logged during
 * debugging: a fresh one nearly every frame), so nothing ever stayed
 * mounted long enough to render. Plain useEffect + useState sidesteps
 * Suspense's retry-from-scratch semantics entirely: the cache below (keyed
 * by URL, same idea as useGLTF's own) still means only the *first* caller
 * for a given URL actually hits the network/parser.
 *
 * Always clones (SkeletonUtils.clone, not a plain .clone() — that doesn't
 * rebind a SkinnedMesh's skeleton correctly) rather than sharing the
 * cached scene directly: the cache is keyed by URL, so two callers loading
 * the *same* model_url — the in-world companion and its settings preview
 * both defaulting to "male-1", say — would otherwise get the exact same
 * live Object3D and fight over its position every frame.
 */
export function useCharacterModel(model_url: string, skin_url: string, height_u: f64): CharacterModel {
  const [loaded, set_loaded] = useState<{ scene: THREE.Group; animations: THREE.AnimationClip[]; skin: THREE.Texture } | null>(null);

  useEffect(() => {
    let cancelled = false;
    set_loaded(null);
    Promise.all([load_gltf(model_url), load_texture(skin_url)]).then(([gltf, skin]) => {
      if (cancelled) return;
      set_loaded({ scene: gltf.scene, animations: gltf.animations, skin });
    });
    return () => {
      cancelled = true;
    };
  }, [model_url, skin_url]);

  const root_ref = useRef<THREE.Group | null>(null);
  const built_for_ref = useRef<{ scene: THREE.Group; skin: THREE.Texture; height_u: f64 } | null>(null);

  if (loaded && (built_for_ref.current?.scene !== loaded.scene || built_for_ref.current?.skin !== loaded.skin || built_for_ref.current?.height_u !== height_u)) {
    const scene = SkeletonUtils.clone(loaded.scene) as THREE.Group;
    const skin = loaded.skin;
    skin.colorSpace = THREE.SRGBColorSpace;
    skin.flipY = false;

    scene.traverse((node) => {
      if (!(node instanceof THREE.Mesh)) return;
      // GLTFLoader tries (and fails — Vite content-hashes the .glb's
      // filename, breaking the embedded relative "Textures/colormap.png"
      // reference) to load its own copy of this texture; overriding here
      // is what actually gets it on screen. MeshBasicMaterial (unlit)
      // because terrain/objects in this renderer are drawn emissive-only
      // (see chunk_stage.tsx's inject_instance_shader) — there's no
      // guarantee of real scene lights bright enough for a lit material.
      node.material = new THREE.MeshBasicMaterial({ map: skin });
    });

    scene.scale.setScalar(1);
    scene.updateMatrixWorld(true);
    const bbox = new THREE.Box3().setFromObject(scene);
    const raw_height = bbox.max.y - bbox.min.y;
    scene.scale.setScalar(raw_height > 0 ? height_u / raw_height : 1);

    // glTF is Y-up by spec; this world is Z-up throughout (get_ele_at_local,
    // every V3, ...). Wrapping rather than rotating `scene` directly keeps
    // the scale/traverse logic above working in the model's own native
    // space.
    const up_axis_fix = new THREE.Group();
    up_axis_fix.rotation.x = Math.PI / 2;
    up_axis_fix.add(scene);

    const group = new THREE.Group();
    group.add(up_axis_fix);

    root_ref.current = group;
    built_for_ref.current = { scene: loaded.scene, skin, height_u };
  }

  // useAnimations needs a real Object3D root on every call (hooks can't be
  // called conditionally) — an empty placeholder before loading finishes,
  // the real (stable-until-reload) group afterward.
  const empty_root_ref = useRef<THREE.Object3D>(new THREE.Object3D());
  const { actions } = useAnimations(loaded?.animations ?? [], root_ref.current ?? empty_root_ref.current);

  return { root: root_ref.current, actions };
}
