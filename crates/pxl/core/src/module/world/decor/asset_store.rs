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

use xpute_core::kit::hash::fnv1a64_str;

use super::decor_kinds::DecorKindBundle;
use super::decor_registry::DECOR_BUNDLES;
use crate::sys::global::Global;
use crate::sys::kernel::kernel;
use crate::module::world::change::{bump_world, DECOR_ASSET};
use crate::render::gltf::load::{load_failed, load_gltf, load_image, load_revision};
use crate::render::gpu::model_asset::register_model;

/// Variants a kind has room for; the palette's largest kind has nine.
const VARIANTS: usize = 16;

#[derive(Clone)]
pub struct DecorBundleAssets {
    pub bundle: &'static DecorKindBundle,
    /// The scheduler's model a variant is, None while it loads.
    pub models: Vec<Option<u32>>,
    /// The shell's image handle a variant is skinned with, None while it loads.
    pub skins: Vec<Option<u32>>,
}

struct Store {
    /// One entry per kind, in registry order, written in place as its
    /// variants' assets arrive: the snapshot is the store itself.
    entries: [DecorBundleAssets; 4],
    revision: u32,
    load_revision: u32,
}

static STORE: Global<Store> = Global::new();

/// Writes what arrived for `url` into every variant naming it, across every
/// kind: two kinds may name the same model or the same skin, and both should
/// reach one object.
fn settle(entries: &mut [DecorBundleAssets], url: &str, value: u32, skin: bool) {
    for entry in entries {
        for (v, variant) in entry.bundle.variants.iter().enumerate() {
            match skin {
                true if variant.skin_url == url => entry.skins[v] = Some(value),
                false if variant.model_url == url => entry.models[v] = Some(value),
                _ => {}
            }
        }
    }
}

fn publish(s: &mut Store, url: &str) {
    s.revision += 1;
    // The group each variant is drawn as, told to the scheduler for its hit test.
    let mut group = 0u32;
    for entry in &s.entries {
        for model in &entry.models {
            if let Some(m) = model {
                kernel().sched.decor_model(group, *m);
            }
            group += 1;
        }
    }
    bump_world(DECOR_ASSET, fnv1a64_str(url));
}

fn store() -> &'static mut Store {
    STORE.get(|| {
        let entry = |bundle: &'static DecorKindBundle| {
            let n = bundle.variants.len();
            assert!(n <= VARIANTS, "decor: {} has {n} variants, room for {VARIANTS}", bundle.kind);
            let none = vec![None; n];
            DecorBundleAssets { bundle, models: none.clone(), skins: none }
        };
        Store {
            entries: DECOR_BUNDLES().map(entry),
            revision: 0,
            load_revision: u32::MAX,
        }
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
    // A model the mesh pool had no room for, this turn. Nothing else will move
    // the load revision on its behalf, so the revision is left unconsumed and
    // the next turn — which may have room — asks again.
    let mut refused = false;
    for b in 0..s.entries.len() {
        let bundle = s.entries[b].bundle;
        for (v, variant) in bundle.variants.iter().enumerate() {
            if s.entries[b].models[v].is_none() && !load_failed(&variant.model_url) {
                if let Some(gltf) = load_gltf(&variant.model_url) {
                    match register_model(&gltf, None) {
                        Some(handle) => {
                            settle(&mut s.entries, &variant.model_url, handle.model, false);
                            publish(s, &variant.model_url);
                        }
                        None => refused = true,
                    }
                }
            }
            if s.entries[b].skins[v].is_none() && !load_failed(&variant.skin_url) {
                if let Some(image) = load_image(&variant.skin_url) {
                    settle(&mut s.entries, &variant.skin_url, image, true);
                    publish(s, &variant.skin_url);
                }
            }
        }
    }
    if !refused {
        s.load_revision = revision;
    }
}

/// What is loaded right now, one entry per decor kind. A variant still in
/// flight reads as None, which is how a consumer knows to leave it out.
pub fn decor_bundle_assets() -> &'static [DecorBundleAssets] {
    &store().entries
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
