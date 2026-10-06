// pxl-kernel/world/chunk/decor/asset_store.rs

//! Every decor kind's loaded models. A model is registered with the scheduler
//! as it arrives, so the eraser's hit test and the renderer read the same
//! lanes. A skin is the load cache's (`load_image`), asked of it where read.

use pxl_spec::asset::ingested::Asset;
use xpute_runtime::sched::edge::{notified, yield_if_spent, Notify};

use crate::host::heap_stat;
use crate::io::load::{load_failed, load_gltf, load_image, Load};
use crate::scene::model::asset::register_model;
use crate::world::change::{request_world_tick, world_edge};
use crate::world::chunk::decor::kind::{Kind, KINDS};
use xpute_runtime::global::Global;

#[derive(Clone)]
pub struct KindAssets {
    pub kind: &'static Kind,
    /// None while it loads.
    pub models: Vec<Option<u32>>,
}

impl KindAssets {
    /// The shell's image handle for a variant's skin; None while it loads.
    pub fn skin(&self, variant: usize) -> Option<u32> {
        load_image(Load::Asset(self.kind.variants.get(variant)?.skin))
    }
}

struct Store {
    /// In `KINDS` order, written in place as assets arrive.
    entries: [KindAssets; 4],
    /// The heap's release count when it last refused a model: not retried until
    /// a block is given back.
    refused: Option<u32>,
}

static STORE: Global<Store> = Global::new();

/// One per asset, since an arrival settles every variant naming it.
pub const MODELS_MAX: usize = pxl_spec::asset::ingested::Asset::ALL.len();

/// Two kinds may name the same model; both reach one object.
fn settle(entries: &mut [KindAssets], asset: Asset, model: u32) {
    for entry in entries {
        for (v, variant) in entry.kind.variants.iter().enumerate() {
            if variant.model == asset {
                entry.models[v] = Some(model);
            }
        }
    }
    request_world_tick();
}

fn store() -> &'static mut Store {
    STORE.get(|| {
        let entry = |kind: &&'static Kind| KindAssets {
            kind,
            models: vec![None; kind.variants.len()],
        };
        Store {
            entries: KINDS.each_ref().map(entry),
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
            let kind = store().entries[b].kind;
            for (v, variant) in kind.variants.iter().enumerate() {
                yield_if_spent(world_edge).await;
                let s = store();
                let model = Load::Asset(variant.model);
                if s.entries[b].models[v].is_none() && !load_failed(model) {
                    if let Some(gltf) = load_gltf(model) {
                        match register_model(&gltf, None) {
                            Some(handle) => settle(&mut s.entries, variant.model, handle.model),
                            None => {
                                refused.get_or_insert(heap_stat().releases);
                            }
                        }
                    }
                }
            }
        }
        store().refused = refused;
        notified(wanted).await;
    }
}

/// A variant still in flight reads as None.
pub fn kind_assets() -> &'static [KindAssets] {
    &store().entries
}

pub fn decor_assets_for(kind: &Kind) -> Option<&'static KindAssets> {
    kind_assets().iter().find(|entry| entry.kind.seed.tag == kind.seed.tag)
}
