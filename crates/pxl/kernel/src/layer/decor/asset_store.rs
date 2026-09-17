// pxl-kernel/layer/decor/asset_store.rs

//! Every decor kind's loaded models and skins, as a plain module store.
//!
//! Arrival is a change like any other: each model or skin that lands is a bump
//! on the world change log's decor-asset topic, and whoever draws the decor
//! re-reads the snapshot at the tick's pump.
//!
//! A model is registered with the scheduler's models as it arrives
//! (io/model_asset.rs), so the kernel holds its triangles for the
//! eraser's hit test (crates/pxl/kernel/src/world/chunk/raycast.rs) and the renderer
//! draws its instances from the same lanes; the group it is drawn as is its
//! variant's place across every kind, in registry order. The registering is
//! `decor_asset_task`'s, a variant at a time on a turn's quota.

use pxl_spec::asset::ingested::Asset;
use xpute_core::sched::edge::{notified, yield_if_spent, Notify};

use crate::io::load::{load_failed, load_gltf, load_image, Load};
use crate::io::model_asset::register_model;
use crate::layer::decor::build::DecorKindBundle;
use crate::layer::decor::registry::DECOR_BUNDLES;
use crate::sys::global::Global;
use crate::sys::kernel::kernel;
use crate::world::change::{bump_world, world_edge, DECOR_ASSET};

/// Variants a kind has room for; kind.json's largest kind has nine.
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
    /// Set when a model or a skin arrives or fails, and kept while a model the
    /// mesh pool refused is still waiting for room.
    arrived: bool,
}

static STORE: Global<Store> = Global::new();

/// Writes what arrived for `asset` into every variant naming it, across every
/// kind: two kinds may name the same model or the same skin, and both should
/// reach one object.
fn settle(entries: &mut [DecorBundleAssets], asset: Asset, value: u32, skin: bool) {
    for entry in entries {
        for (v, variant) in entry.bundle.variants.iter().enumerate() {
            match skin {
                true if variant.skin == asset => entry.skins[v] = Some(value),
                false if variant.model == asset => entry.models[v] = Some(value),
                _ => {}
            }
        }
    }
}

fn publish(s: &mut Store, asset: Asset) {
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
    bump_world(DECOR_ASSET, asset as u64);
}

fn store() -> &'static mut Store {
    STORE.get(|| {
        let entry = |bundle: &'static DecorKindBundle| {
            let n = bundle.variants.len();
            assert!(n <= VARIANTS, "decor: {:?} has {n} variants, room for {VARIANTS}", bundle.kind);
            let none = vec![None; n];
            DecorBundleAssets {
                bundle,
                models: none.clone(),
                skins: none,
            }
        };
        Store {
            entries: DECOR_BUNDLES().map(entry),
            arrived: true,
        }
    })
}

/// A model or a skin arrived or failed.
pub fn decor_assets_arrived() {
    store().arrived = true;
}

static WANTED: Global<Notify> = Global::new();

fn wanted() -> &'static mut Notify {
    WANTED.get(Notify::new)
}

/// The frame's check: something arrived since the task last ran, so the task
/// is asked to run.
pub fn decor_assets_wanted() {
    if store().arrived {
        wanted().notify();
    }
}

/// The store's task: asks for every variant's model and skin, and registers
/// whatever arrived, a variant at a time on a turn's quota.
pub async fn decor_asset_task() {
    loop {
        notified(wanted).await;
        store().arrived = false;
        // A model the mesh pool had no room for: the flag is set again, and
        // the frame after — which may have room — asks again.
        let mut refused = false;
        for b in 0..store().entries.len() {
            let bundle = store().entries[b].bundle;
            for (v, variant) in bundle.variants.iter().enumerate() {
                yield_if_spent(world_edge).await;
                let s = store();
                let (model, skin) = (Load::Asset(variant.model), Load::Asset(variant.skin));
                if s.entries[b].models[v].is_none() && !load_failed(model) {
                    if let Some(gltf) = load_gltf(model) {
                        match register_model(&gltf, None) {
                            Some(handle) => {
                                settle(&mut s.entries, variant.model, handle.model, false);
                                publish(s, variant.model);
                            }
                            None => refused = true,
                        }
                    }
                }
                if s.entries[b].skins[v].is_none() && !load_failed(skin) {
                    if let Some(image) = load_image(skin) {
                        settle(&mut s.entries, variant.skin, image, true);
                        publish(s, variant.skin);
                    }
                }
            }
        }
        if refused {
            store().arrived = true;
        }
    }
}

/// What is loaded right now, one entry per decor kind. A variant still in
/// flight reads as None, which is how a consumer knows to leave it out.
pub fn decor_bundle_assets() -> &'static [DecorBundleAssets] {
    &store().entries
}

/// One kind's assets, or None while nothing of it has loaded yet.
pub fn decor_assets_for(bundle: Option<&DecorKindBundle>, assets: &'static [DecorBundleAssets]) -> Option<&'static DecorBundleAssets> {
    let bundle = bundle?;
    assets.iter().find(|entry| core::ptr::eq(entry.bundle, bundle))
}
