// pxl-kernel/scene/billboard/field.rs

//! Billboards: labels and bubbles drawn flat to the camera, as sets of
//! instances. A set stands at an origin, or a height above an actor (a chat
//! bubble).

use pxl_render::frame::draw::{DrawList, Held, PIPELINE_BADGE};
use pxl_render::gpu::api::bytes;
use pxl_spec::space::coord::v3::V3;
use xpute_guest::abi::handle::Slots;

use crate::scene::actor::Actors;
use crate::scene::billboard::sheet::billboard_sheet;

/// Anchor relative to the set's (3), logical size in pixels (2), sheet
/// rectangle (4), anchor's place down the sprite, 0 top 1 bottom (1).
/// badge.wgsl reads the same stride.
pub const BILLBOARD_FLOATS: usize = 10;

/// One instance: its anchor `at`, `size` in logical pixels, the sheet's
/// `slot`, and where down the sprite the anchor stands.
pub fn billboard_instance(at: V3, size: [f64; 2], slot: u32, down: f64) -> [f32; BILLBOARD_FLOATS] {
    let sheet = billboard_sheet();
    [at[0], at[1], at[2], size[0], size[1], sheet.u0(slot), sheet.v0(slot), sheet.u1(slot), sheet.v1(slot), down].map(|v| v as f32)
}

#[derive(Clone, Copy)]
enum Anchor {
    At(V3),
    On(u32, f64),
}

struct Set {
    /// A later order wins a tie on the same pixels at the same depth.
    order: u32,
    anchor: Anchor,
    held: Held,
    count: u32,
}

#[derive(Default)]
pub struct Billboards {
    sets: Slots<Set>,
}

impl Billboards {
    pub fn new_set(&mut self, order: u32) -> u32 {
        // One set per order (sheet.rs `SETS_MAX`).
        xpute_core::ensure!(self.sets.iter().all(|(_, s)| s.order != order), ENOTRECOVERABLE, order);
        let set = Set {
            order,
            anchor: Anchor::At([0.0; 3]),
            held: Held::default(),
            count: 0,
        };
        self.sets.insert(set)
    }

    pub fn write(&mut self, draw: &mut DrawList, id: u32, lane: &[f32]) {
        let Some(s) = self.sets.get_mut(id) else { return };
        let written = s.held.write(draw, bytes(lane));
        s.count = if written { (lane.len() / BILLBOARD_FLOATS) as u32 } else { 0 };
    }

    pub fn place(&mut self, id: u32, origin: V3) {
        if let Some(s) = self.sets.get_mut(id) {
            s.anchor = Anchor::At(origin);
        }
    }

    pub fn follow(&mut self, id: u32, actor: u32, lift: f64) {
        if let Some(s) = self.sets.get_mut(id) {
            s.anchor = Anchor::On(actor, lift);
        }
    }

    pub fn remove(&mut self, draw: &mut DrawList, id: u32) {
        if let Some(mut s) = self.sets.remove(id) {
            s.held.free(draw);
        }
    }

    /// A set on an actor that is gone or hidden shows nothing.
    pub fn draw(&self, draw: &mut DrawList, actors: &Actors, eye: V3) {
        // A pass per order over the few there are, so no list is sorted per frame.
        let orders = || self.sets.iter().map(|(_, s)| s.order);
        let mut next = orders().min();
        while let Some(order) = next {
            for (_, s) in self.sets.iter().filter(|(_, s)| s.order == order) {
                if s.count == 0 || s.held.handle == 0 {
                    continue;
                }
                let at = match s.anchor {
                    Anchor::At(at) => at,
                    Anchor::On(actor, lift) => match actors.actor_ref(actor) {
                        Some(a) if a.visible => [a.at[0], a.at[1], a.at[2] + lift],
                        _ => continue,
                    },
                };
                draw.draw_as(PIPELINE_BADGE, s.held.handle, s.count, 0, DrawList::translation(at, eye), 0.0, 1.0);
            }
            next = orders().filter(|&o| o > order).min();
        }
    }
}
