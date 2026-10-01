// pxl-kernel/layer/decor/asset_store.rs

//! Every decor kind's loaded models and skins, as a plain module store.
//!
//! Whoever draws the decor reads the store itself each tick, so an arrival
//! needs no record of its own: each model or skin that lands asks for the
//! frame that draws it.
//!
//! A model is registered with the scheduler's models as it arrives
//! (layer/model/asset.rs), so the kernel holds its triangles for the
//! eraser's hit test (layer/decor/hit.rs) and the renderer
//! draws its instances from the same lanes; the group it is drawn as is its
//! variant's place across every kind, in registry order. The registering is
//! `decor_asset_task`'s, a variant at a time on a turn's quota.

use pxl_spec::asset::ingested::Asset;
use xpute_runtime::sched::edge::{notified, yield_if_spent, Notify};

use crate::global::Global;
use crate::host::heap_stat;
use crate::io::load::{load_failed, load_gltf, load_image, Load};
use crate::layer::decor::build::DecorKindBundle;
use crate::layer::decor::registry::DECOR_BUNDLES;
use crate::layer::model::asset::register_model;
use crate::world::change::{request_world_tick, world_edge};

/// Variants a kind has room for; kind.rs's largest kind has nine.
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
    /// The heap's release count when it last refused a model: until a block
    /// is given back, the same model meets the same heap, and it is not tried.
    refused: Option<u32>,
}

static STORE: Global<Store> = Global::new();

/// Models decor registers at most: one an asset, since an arrival is
/// settled into every variant naming it (`settle`) and a variant with its
/// model asks for none.
pub const MODELS_MAX: usize = pxl_spec::asset::ingested::Asset::ALL.len();

/// Writes what arrived for `asset` into every variant naming it, across every
/// kind: two kinds may name the same model or the same skin, and both should
/// reach one object; and the frame that draws what arrived is asked for.
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
    request_world_tick();
}

fn store() -> &'static mut Store {
    STORE.get(|| {
        let entry = |bundle: &'static DecorKindBundle| {
            let n = bundle.variants.len();
            xpute_core::ensure!(n <= VARIANTS, ENOSPC, bundle.seed.tag, n);
            let none = vec![None; n];
            DecorBundleAssets {
                bundle,
                models: none.clone(),
                skins: none,
            }
        };
        Store {
            entries: DECOR_BUNDLES().each_ref().map(entry),
            refused: None,
        }
    })
}

static WANTED: Global<Notify> = Global::new();

fn wanted() -> &'static mut Notify {
    WANTED.get(Notify::new)
}

/// A model or a skin arrived or failed: the task looks at once.
pub fn decor_assets_arrived() {
    wanted().notify();
}

/// The frame's check: a model the heap refused is tried again once the heap
/// has given a block back.
pub fn decor_assets_wanted() {
    let s = store();
    if s.refused.is_some_and(|r| r != heap_stat().releases) {
        s.refused = None;
        wanted().notify();
    }
}

/// The store's task: asks for every variant's model and skin, and registers
/// whatever arrived, a variant at a time on a turn's quota — at once, which
/// asks for them all, and again whenever something arrives or the heap has
/// room for a model it refused.
pub async fn decor_asset_task() {
    loop {
        let mut refused = None;
        for b in 0..store().entries.len() {
            let bundle = store().entries[b].bundle;
            for (v, variant) in bundle.variants.iter().enumerate() {
                yield_if_spent(world_edge).await;
                let s = store();
                let (model, skin) = (Load::Asset(variant.model), Load::Asset(variant.skin));
                if s.entries[b].models[v].is_none() && !load_failed(model) {
                    if let Some(gltf) = load_gltf(model) {
                        match register_model(&gltf, None) {
                            Some(handle) => settle(&mut s.entries, variant.model, handle.model, false),
                            None => {
                                refused.get_or_insert(heap_stat().releases);
                            }
                        }
                    }
                }
                if s.entries[b].skins[v].is_none() && !load_failed(skin) {
                    if let Some(image) = load_image(skin) {
                        settle(&mut s.entries, variant.skin, image, true);
                    }
                }
            }
        }
        store().refused = refused;
        notified(wanted).await;
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
