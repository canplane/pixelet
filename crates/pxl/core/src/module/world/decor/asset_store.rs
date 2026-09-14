// pxl_core/module/world/decor/asset_store.rs

//! Every decor kind's loaded models and skins, as a plain module store.
//!
//! Arrival is a change like any other: each model or skin that lands is a bump
//! on the world change log's decor-asset topic, and whoever draws the decor
//! re-reads the snapshot at the tick's pump.
//!
//! A model is registered with the scheduler's models as it arrives
//! (render/gpu/model_asset.rs), so the kernel holds its triangles for the
//! eraser's hit test (crates/pxl/core/src/world/raycast.rs) and the renderer
//! draws its instances from the same lanes; the group it is drawn as is its
//! variant's place across every kind, in registry order.

use std::collections::HashMap;

use xpute_core::kit::hash::fnv1a64_str;

use super::decor_kinds::DecorKindBundle;
use super::decor_registry::DECOR_BUNDLES;
use crate::global::Global;
use crate::kernel::kernel;
use crate::module::world::change::{bump_world, DECOR_ASSET};
use crate::render::gltf::load::{load_failed, load_gltf, load_image, load_revision};
use crate::render::gpu::model_asset::{register_model, ModelHandle};

#[derive(Clone, Debug, PartialEq)]
pub struct DecorBundleAssets {
    pub bundle: &'static DecorKindBundle,
    /// The scheduler's model a variant is, None while it loads.
    pub models: Vec<Option<ModelHandle>>,
    /// The shell's image handle a variant is skinned with, None while it loads.
    pub skins: Vec<Option<u32>>,
}

struct Store {
    /// Keyed by URL, not by variant: two kinds may name the same model or the
    /// same skin, and both should reach one object.
    model_by_url: HashMap<String, Option<ModelHandle>>,
    skin_by_url: HashMap<String, Option<u32>>,
    /// Rebuilt whole on every arrival; its revision moves with it.
    snapshot: Vec<DecorBundleAssets>,
    revision: u32,
    load_revision: u32,
}

static STORE: Global<Store> = Global::new();

fn build_snapshot(s: &Store) -> Vec<DecorBundleAssets> {
    DECOR_BUNDLES()
        .into_iter()
        .map(|bundle| DecorBundleAssets {
            bundle,
            models: bundle.variants.iter().map(|v| s.model_by_url.get(&v.model_url).cloned().flatten()).collect(),
            skins: bundle.variants.iter().map(|v| s.skin_by_url.get(&v.skin_url).copied().flatten()).collect(),
        })
        .collect()
}

fn publish(s: &mut Store, url: &str) {
    s.snapshot = build_snapshot(s);
    s.revision += 1;
    // The group each variant is drawn as, told to the scheduler for its hit test.
    let mut group = 0u32;
    for entry in &s.snapshot {
        for model in &entry.models {
            if let Some(m) = model {
                kernel().sched.decor_model(group, m.model);
            }
            group += 1;
        }
    }
    bump_world(DECOR_ASSET, fnv1a64_str(url));
}

fn store() -> &'static mut Store {
    STORE.get(|| {
        let mut s = Store {
            model_by_url: HashMap::new(),
            skin_by_url: HashMap::new(),
            snapshot: Vec::new(),
            revision: 0,
            load_revision: u32::MAX,
        };
        s.snapshot = build_snapshot(&s);
        s
    })
}

/// The store's turn: asks for every variant's model and skin, and registers
/// whatever arrived since the last turn.
pub fn decor_asset_step() {
    let s = store();
    let revision = load_revision();
    if s.load_revision == revision {
        return;
    }
    s.load_revision = revision;
    for bundle in DECOR_BUNDLES() {
        for variant in &bundle.variants {
            if !matches!(s.model_by_url.get(&variant.model_url), Some(Some(_))) && !load_failed(&variant.model_url) {
                if let Some(gltf) = load_gltf(&variant.model_url) {
                    let handle = register_model(&gltf, None);
                    s.model_by_url.insert(variant.model_url.clone(), handle);
                    publish(s, &variant.model_url);
                } else {
                    s.model_by_url.entry(variant.model_url.clone()).or_insert(None);
                }
            }
            if !matches!(s.skin_by_url.get(&variant.skin_url), Some(Some(_))) && !load_failed(&variant.skin_url) {
                if let Some(image) = load_image(&variant.skin_url) {
                    s.skin_by_url.insert(variant.skin_url.clone(), Some(image));
                    publish(s, &variant.skin_url);
                } else {
                    s.skin_by_url.entry(variant.skin_url.clone()).or_insert(None);
                }
            }
        }
    }
}

/// What is loaded right now, one entry per decor kind. A variant still in
/// flight reads as None, which is how a consumer knows to leave it out.
pub fn decor_bundle_assets() -> &'static [DecorBundleAssets] {
    &store().snapshot
}

/// Moves on every arrival.
pub fn decor_assets_revision() -> u32 {
    store().revision
}

/// One kind's assets, or None while nothing of it has loaded yet.
pub fn decor_assets_for(bundle: Option<&DecorKindBundle>, assets: &'static [DecorBundleAssets]) -> Option<&'static DecorBundleAssets> {
    let bundle = bundle?;
    assets.iter().find(|entry| core::ptr::eq(entry.bundle, bundle))
}
