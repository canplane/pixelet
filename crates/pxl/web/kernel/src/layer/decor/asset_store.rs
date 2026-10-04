// pxl-kernel/layer/decor/asset_store.rs

//! Every decor kind's loaded models and skins. A model is registered with
//! the scheduler as it arrives, so the eraser's hit test and the renderer
//! read the same lanes.

use pxl_spec::asset::ingested::Asset;
use xpute_runtime::sched::edge::{notified, yield_if_spent, Notify};

use crate::host::heap_stat;
use crate::io::load::{load_failed, load_gltf, load_image, Load};
use crate::layer::decor::build::DecorKindBundle;
use crate::layer::decor::registry::DECOR_BUNDLES;
use crate::layer::model::asset::register_model;
use crate::world::change::{request_world_tick, world_edge};
use xpute_runtime::global::Global;

/// kind.rs's largest kind has nine.
const VARIANTS: usize = 16;

#[derive(Clone)]
pub struct DecorBundleAssets {
    pub bundle: &'static DecorKindBundle,
    /// None while it loads.
    pub models: Vec<Option<u32>>,
    /// The shell's image handle; None while it loads.
    pub skins: Vec<Option<u32>>,
}

struct Store {
    /// In registry order, written in place as assets arrive.
    entries: [DecorBundleAssets; 4],
    /// The heap's release count when it last refused a model: not retried until
    /// a block is given back.
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

pub fn decor_assets_arrived() {
    wanted().notify();
}

pub fn decor_assets_wanted() {
    let s = store();
    if s.refused.is_some_and(|r| r != heap_stat().releases) {
        s.refused = None;
        wanted().notify();
    }
}

/// Registers whatever arrived, a variant at a time on the turn quota.
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

/// A variant still in flight reads as None.
pub fn decor_bundle_assets() -> &'static [DecorBundleAssets] {
    &store().entries
}

pub fn decor_assets_for(bundle: Option<&DecorKindBundle>, assets: &'static [DecorBundleAssets]) -> Option<&'static DecorBundleAssets> {
    let bundle = bundle?;
    assets.iter().find(|entry| core::ptr::eq(entry.bundle, bundle))
}
