// @pxl/web/modules/world/decor/asset_store.ts

/**
 * Every decor kind's loaded models, as a plain module store.
 *
 * These used to load through a React hook whose result reached the chunk
 * pipeline as a prop: useAllDecorBundleAssets -> ChunkStage -> PageChunks ->
 * new ChunkNode(). That put React on the path of a fact nothing about React
 * is needed to know — "this model has arrived" — and a node already
 * constructed never heard it, because the effect that would have delivered
 * it only created nodes that did not exist yet. Chunks built before the
 * models finished loading stayed decor-free for their whole lives, so decor
 * appeared or did not depending on whether the browser cache beat the first
 * chunk build.
 *
 * Here arrival is a notification like any other source change, and a
 * ChunkNode subscribes to it exactly the way it already subscribes to tiles
 * and to the object store. Nothing has to be told twice.
 *
 * Kept out of instanced_model.ts on purpose: this module reaches
 * decor_registry.ts and settings_store.ts, whose `*.glb?url` imports
 * `deno test` has no loader for, and instanced_model.ts is imported
 * directly by tests.
 */

import { useSyncExternalStore } from "react";
import * as THREE from "three";

import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { rim_cfg } from "@/modules/world/environment/config.ts";
import { set_rim_intensity } from "@/modules/world/render/lit_gltf_material.ts";
import { load_gltf, load_texture } from "@/modules/world/gltf_cache.ts";
import { make_lit_gltf_material } from "@/modules/world/render/lit_gltf_material.ts";
import { type DecorVariantAsset, merge_scene_geometry } from "./instanced_model.ts";
import type { DecorKindBundle } from "./decor_layer.ts";
import { DECOR_BUNDLES } from "./decor_registry.ts";

export type DecorBundleAssets = {
  bundle: DecorKindBundle;
  geometries: readonly (THREE.BufferGeometry | null)[];
  materials: readonly (THREE.MeshStandardMaterial | null)[];
};

/** Keyed by URL, not by variant: two kinds may name the same model or the
 * same skin, and both should reach one THREE object. */
const geometry_by_url = new Map<string, THREE.BufferGeometry | null>();
const material_by_url = new Map<string, THREE.MeshStandardMaterial | null>();

const listeners = new Set<() => void>();

/** Rebuilt whole on every arrival so a consumer can tell "changed" by
 * identity, the way a revision number would. Arrivals are a few dozen
 * events across a session, not a per-frame stream. */
let snapshot: readonly DecorBundleAssets[] = build_snapshot();

function build_snapshot(): readonly DecorBundleAssets[] {
  return DECOR_BUNDLES.map((bundle) => ({
    bundle,
    geometries: bundle.variants.map((variant) => geometry_by_url.get(variant.model_url) ?? null),
    materials: bundle.variants.map((variant) => material_by_url.get(variant.skin_url) ?? null),
  }));
}

function publish(): void {
  snapshot = build_snapshot();
  for (const on_change of listeners) on_change();
}

function rim_intensity(): number {
  return useSettingsStore.getState().show_rim_light ? rim_cfg.INTENSITY : 0;
}

function load_variant(variant: DecorVariantAsset): void {
  if (!geometry_by_url.has(variant.model_url)) {
    geometry_by_url.set(variant.model_url, null);
    load_gltf(variant.model_url).then((gltf) => {
      geometry_by_url.set(variant.model_url, merge_scene_geometry(gltf.scene));
      publish();
    });
  }
  if (!material_by_url.has(variant.skin_url)) {
    material_by_url.set(variant.skin_url, null);
    load_texture(variant.skin_url).then((texture) => {
      texture.colorSpace = THREE.SRGBColorSpace;
      texture.flipY = false;
      // Per skin rather than one key for every decor material.
      //
      // The shared key was there because the compiled program is identical
      // for all of them, which is true of the shader source and not of what
      // three.js does with it: a material that finds a program already
      // cached under the key never compiles, so its onBeforeCompile never
      // runs and — the part that matters — it inherits whatever defines the
      // first material to compile had. make_lit_gltf_material sets
      // `emissive: 0xffffff` with an emissiveMap to modulate it, so a
      // program without USE_EMISSIVEMAP renders that white literally, which
      // is exactly what a white model is.
      //
      // Confirmed by the symptom stopping. The cost is one compiled program
      // per skin rather than one in total, which at five skins is nothing —
      // and the sharing it gave up was mostly illusory, since three.js keys
      // programs by the standard parameters too and these materials would
      // have shared one anyway.
      const material = make_lit_gltf_material(texture, `decor-mesh:${variant.skin_url}`);
      set_rim_intensity(material, rim_intensity());
      material_by_url.set(variant.skin_url, material);
      publish();
    });
  }
}

for (const bundle of DECOR_BUNDLES) {
  for (const variant of bundle.variants) load_variant(variant);
}

// One subscription for every decor material there will ever be, rather than
// one per consumer: the material objects are shared across every chunk that
// draws that variant, so the uniform is written once here no matter how many
// chunks are on screen.
useSettingsStore.subscribe(() => {
  const intensity = rim_intensity();
  for (const material of material_by_url.values()) {
    if (material) set_rim_intensity(material, intensity);
  }
});

/** What is loaded right now, one entry per decor kind. A variant still in
 * flight reads as null, which is how a consumer knows to leave it out. */
export function decor_bundle_assets(): readonly DecorBundleAssets[] {
  return snapshot;
}

export function subscribe_decor_assets(on_change: () => void): () => void {
  listeners.add(on_change);
  return () => listeners.delete(on_change);
}

/** The same store, for the handful of React consumers that need it — the
 * hover ghost's decor silhouette. React subscribes to the store; the store
 * never waits on React. */
export function useDecorAssets(): readonly DecorBundleAssets[] {
  return useSyncExternalStore(subscribe_decor_assets, decor_bundle_assets);
}

/** One kind's assets, or null while nothing of it has loaded yet. */
export function decor_assets_for(bundle: DecorKindBundle | null, assets: readonly DecorBundleAssets[]): DecorBundleAssets | null {
  if (!bundle) return null;
  return assets.find((entry) => entry.bundle === bundle) ?? null;
}
