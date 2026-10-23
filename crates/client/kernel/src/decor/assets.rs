// pxl-kernel/decor/assets.rs

//! Every decor's loaded models. A model is registered with the scheduler
//! as it arrives, so the pick (`decor/strike.rs`) and the renderer read the
//! same mesh. A skin is the load cache's (`load_image`), asked of it where read.

use pxl_spec::asset::ingested::Asset;
use xpute_guest::sched::edge::{notified, yield_if_spent, Notify};

use crate::asset::Load;
use crate::io::host::Refused;
use crate::io::load::{load_failed, load_gltf, load_image, take_settled};
use crate::model::asset::register_model;
use crate::sched::edge::{request_world_tick, world_edge};
use pxl_spec::block::decor::{Decor, DECORS};
use xpute_guest::global::Global;

#[derive(Clone)]
pub struct DecorAssets {
    pub decor: &'static Decor,
    /// None while it loads.
    pub models: Vec<Option<u32>>,
}

impl DecorAssets {
    /// A variant's skin as the renderer takes it (`load_image`); None while
    /// it loads.
    pub fn skin(&self, variant: usize) -> Option<(u32, &'static [u8])> {
        load_image(Load::Asset(self.decor.variants.get(variant)?.skin))
    }
}

struct Store {
    /// In `DECORS` order, written in place as assets arrive.
    entries: [DecorAssets; 4],
    /// The heap's last refusal of a model: not retried until a block is
    /// given back.
    refused: Option<Refused>,
}

static STORE: Global<Store> = Global::new();

/// One per asset, since an arrival settles every variant naming it.
pub const MODELS_MAX: usize = pxl_spec::asset::ingested::Asset::ALL.len();

/// Two decors may name the same model; both reach one object.
fn settle(entries: &mut [DecorAssets], asset: Asset, model: u32) {
    for entry in entries {
        for (v, variant) in entry.decor.variants.iter().enumerate() {
            if variant.model == asset {
                entry.models[v] = Some(model);
            }
        }
    }
    request_world_tick();
}

fn store() -> &'static mut Store {
    STORE.get(|| {
        let entry = |decor: &'static Decor| DecorAssets {
            decor,
            models: vec![None; decor.variants.len()],
        };
        Store {
            entries: DECORS.each_ref().map(entry),
            refused: None,
        }
    })
}

static WANTED: Global<Notify> = Global::new();

fn wanted() -> &'static mut Notify {
    WANTED.get(Notify::new)
}

pub fn decor_assets_wanted() {
    let s = store();
    if take_settled() {
        wanted().notify();
    }
    if s.refused.is_some_and(Refused::over) {
        s.refused = None;
        wanted().notify();
    }
}

/// Registers whatever arrived, a variant at a time on the turn quota.
pub async fn decor_asset_task() {
    loop {
        let mut refused = None;
        for b in 0..store().entries.len() {
            let decor = store().entries[b].decor;
            for (v, variant) in decor.variants.iter().enumerate() {
                yield_if_spent(world_edge).await;
                let s = store();
                let model = Load::Asset(variant.model);
                if s.entries[b].models[v].is_none() && !load_failed(model) {
                    if let Some(gltf) = load_gltf(model) {
                        match register_model(&gltf, None) {
                            Some(handle) => settle(&mut s.entries, variant.model, handle.model),
                            None => {
                                refused.get_or_insert_with(Refused::now);
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
pub fn decor_assets() -> &'static [DecorAssets] {
    &store().entries
}

pub fn decor_assets_for(decor: &Decor) -> Option<&'static DecorAssets> {
    decor_assets().iter().find(|entry| entry.decor.id == decor.id)
}
