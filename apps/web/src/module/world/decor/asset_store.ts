// @pxl/web/module/world/decor/asset_store.ts

/**
 * Every decor kind's loaded models and skins, as a plain module store.
 *
 * These used to load through a React hook whose result reached the chunk
 * pipeline as a prop, and a node already constructed never heard it —
 * chunks built before the models finished loading stayed decor-free for
 * their whole lives. Here arrival is a change like any other: each model
 * or skin that lands is a bump on the world change log's decor-asset
 * topic, and whoever draws the decor re-reads the snapshot at the tick's
 * pump.
 *
 * A model is registered with the module as it arrives (render/gpu/
 * model_asset.ts), so the core holds its triangles for the eraser's hit
 * test (crates/pxl-core/src/world/raycast.rs) and the renderer draws its
 * instances from the same lanes; the group it is drawn as is its
 * variant's place across every kind, in registry order.
 */

import { useSyncExternalStore } from "react";

import type { u32 } from "@xpute/core/kit/word.ts";
import { fnv1a64_str } from "@xpute/core/kit/hash.ts";

import { load_gltf, load_image } from "@/render/gltf/load.ts";
import { type ModelHandle, register_model } from "@/render/gpu/model_asset.ts";
import { CORE_PXL } from "@/render/layer/pipeline/core_pool.ts";
import { bump_world, world_changes, world_wakers, WorldTopic } from "@/module/world/change.ts";
import type { DecorKindBundle, DecorVariantAsset } from "./decor_layer.ts";
import { DECOR_BUNDLES } from "./decor_registry.ts";

export type DecorBundleAssets = {
  bundle: DecorKindBundle;
  /** The module's model a variant is, null while it loads. */
  models: readonly (ModelHandle | null)[];
  skins: readonly (ImageBitmap | null)[];
};

/** Keyed by URL, not by variant: two kinds may name the same model or the
 * same skin, and both should reach one object. */
const model_by_url = new Map<string, ModelHandle | null>();
const skin_by_url = new Map<string, ImageBitmap | null>();

/** Rebuilt whole on every arrival so a consumer can tell "changed" by
 * identity, the way a revision number would. Arrivals are a few dozen
 * events across a session, not a per-frame stream. */
let snapshot: readonly DecorBundleAssets[] = build_snapshot();

function build_snapshot(): readonly DecorBundleAssets[] {
  return DECOR_BUNDLES.map((bundle) => ({
    bundle,
    models: bundle.variants.map((variant) => model_by_url.get(variant.model_url) ?? null),
    skins: bundle.variants.map((variant) => skin_by_url.get(variant.skin_url) ?? null),
  }));
}

function publish(url: string): void {
  snapshot = build_snapshot();
  // The group each variant is drawn as, told to the module for its hit test.
  const p = CORE_PXL;
  if (p) {
    let group: u32 = 0;
    for (const { models } of snapshot) {
      for (const model of models) {
        if (model) p.pxl_decor_model(group, model.model);
        group++;
      }
    }
  }
  bump_world(WorldTopic.DECOR_ASSET, fnv1a64_str(url));
}

function load_variant(variant: DecorVariantAsset): void {
  if (!model_by_url.has(variant.model_url)) {
    model_by_url.set(variant.model_url, null);
    load_gltf(variant.model_url).then((gltf) => {
      const p = CORE_PXL;
      if (!p) return;
      model_by_url.set(variant.model_url, register_model(p, gltf, null));
      publish(variant.model_url);
    });
  }
  if (!skin_by_url.has(variant.skin_url)) {
    skin_by_url.set(variant.skin_url, null);
    load_image(variant.skin_url).then((image) => {
      skin_by_url.set(variant.skin_url, image);
      publish(variant.skin_url);
    });
  }
}

for (const bundle of DECOR_BUNDLES) {
  for (const variant of bundle.variants) load_variant(variant);
}

/** What is loaded right now, one entry per decor kind. A variant still in
 * flight reads as null, which is how a consumer knows to leave it out. */
export function decor_bundle_assets(): readonly DecorBundleAssets[] {
  return snapshot;
}

/** Calls `on_change` once for a pump in which any asset arrived, however
 * many did: every consumer re-reads the whole snapshot anyway. */
export function subscribe_decor_assets(on_change: () => void): () => void {
  let heard = world_changes.tick(WorldTopic.DECOR_ASSET);
  return world_wakers.subscribe_topic(WorldTopic.DECOR_ASSET, () => {
    const tick = world_changes.tick(WorldTopic.DECOR_ASSET);
    if (tick === heard) return;
    heard = tick;
    on_change();
  });
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
