// pxl-kernel/decor/assets.rs

//! Every decor's loaded models. A model is registered with the scheduler
//! as it arrives, so the pick (`decor/strike.rs`) and the renderer read the
//! same mesh. A skin is the load cache's (`load_image`), asked of it where read.

use pxl_spec::asset::ingested::Asset;
use xpute_guest::sched::edge::{notified, yield_if_spent, Notify};

use crate::asset::Load;
use crate::io::load::{load_image, take_settled};
use crate::model::asset::{model_of, model_refusal_over};
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

/// In `DECORS` order, written in place as assets arrive.
static STORE: Global<[DecorAssets; 4]> = Global::new();

/// One per asset: two decors naming the same model reach one (`model_of`).
pub const MODELS_MAX: usize = Asset::ALL.len();

fn store() -> &'static mut [DecorAssets; 4] {
    STORE.get(|| {
        DECORS.each_ref().map(|decor| DecorAssets {
            decor,
            models: vec![None; decor.variants.len()],
        })
    })
}

static WANTED: Global<Notify> = Global::new();

fn wanted() -> &'static mut Notify {
    WANTED.get(Notify::new)
}

/// Wakes the task when a load arrived, or when the heap gave room back to a
/// model it refused.
pub fn decor_assets_wanted() {
    let refusal_over = || {
        store().iter().any(|entry| {
            entry
                .decor
                .variants
                .iter()
                .zip(&entry.models)
                .any(|(variant, model)| model.is_none() && model_refusal_over(variant.model, None))
        })
    };
    if take_settled() || refusal_over() {
        wanted().notify();
    }
}

/// Registers whatever arrived, a variant at a time on the turn quota.
pub async fn decor_asset_task() {
    loop {
        for b in 0..store().len() {
            let decor = store()[b].decor;
            for (v, variant) in decor.variants.iter().enumerate() {
                yield_if_spent(world_edge).await;
                let slot = &mut store()[b].models[v];
                if slot.is_none() {
                    *slot = model_of(variant.model, None).map(|handle| handle.model);
                    if slot.is_some() {
                        request_world_tick();
                    }
                }
            }
        }
        notified(wanted).await;
    }
}

/// A variant still in flight reads as None.
pub fn decor_assets() -> &'static [DecorAssets] {
    store()
}

pub fn decor_assets_for(decor: &Decor) -> Option<&'static DecorAssets> {
    decor_assets().iter().find(|entry| entry.decor.id == decor.id)
}
