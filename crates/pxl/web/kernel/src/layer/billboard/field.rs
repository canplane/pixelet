// pxl-kernel/layer/billboard/field.rs

//! Billboards: labels and bubbles drawn flat to the camera over the world,
//! as sets of instances in one layout. A set stands either at an origin —
//! every label on screen, replaced whole when a placement commits — or on
//! an actor, a height above wherever the actor stands this tick, which is
//! what a chat bubble over the companion is.
//! What a set shows is its layer's (badge.rs, chat_bubble.rs: the sheet,
//! the rectangles, the sizes);
//! the core holds the instances as a block and lists each set's draw,
//! placed against the frame's origin, in the order the sets draw in.

use pxl_render::frame::draw::{DrawList, Held, PIPELINE_BADGE};
use pxl_render::gpu::api::word_bytes;
use pxl_spec::space::v3::V3;
use xpute_runtime::abi::handle::Slots;

use crate::layer::model::store::Models;
use crate::world::chunk::node::World;

/// Floats an instance takes: its anchor relative to the set's (3), its
/// logical size in pixels (2), its rectangle of the sheet (4), and where the
/// anchor falls down the sprite, 0 top and 1 bottom (1). badge.wgsl reads
/// the same stride.
pub const BILLBOARD_FLOATS: usize = 10;

#[derive(Clone, Copy)]
enum Anchor {
    At(V3),
    /// An actor, and how far above where it stands.
    On(u32, f64),
}

struct Set {
    /// Draws after every set of a lower order: a tie on the same pixels at
    /// the same depth goes to the later one.
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
    /// A set, empty and standing at the origin, drawn in `order`.
    pub fn new_set(&mut self, order: u32) -> u32 {
        let set = Set {
            order,
            anchor: Anchor::At([0.0; 3]),
            held: Held::default(),
            count: 0,
        };
        self.sets.insert(set)
    }

    /// Replaces a set's instances with `count` of BILLBOARD_FLOATS floats
    /// from `lane`.
    ///
    /// # Safety
    /// `lane` addresses count * BILLBOARD_FLOATS f32s in the memory.
    pub unsafe fn write(&mut self, world: &mut World, draw: &mut DrawList, id: u32, count: u32, lane: usize) {
        let Some(s) = self.sets.get_mut(id) else { return };
        let len = word_bytes(count as usize * BILLBOARD_FLOATS);
        let written = s.held.write(draw, world.volumes.mem(), lane, len);
        s.count = if written { count } else { 0 };
    }

    /// Stands a set at `origin`, in units.
    pub fn place(&mut self, id: u32, origin: V3) {
        if let Some(s) = self.sets.get_mut(id) {
            s.anchor = Anchor::At(origin);
        }
    }

    /// Stands a set `lift` units above wherever `actor` stands.
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

    /// Every set with something to show, in order; a set on an actor that
    /// is gone or hidden shows nothing.
    pub fn draw(&self, draw: &mut DrawList, models: &Models, origin: V3) {
        // By order, and within one in the order their slots lie: a pass an
        // order over the few there are, so a frame makes no list to sort.
        let orders = || self.sets.iter().map(|(_, s)| s.order);
        let mut next = orders().min();
        while let Some(order) = next {
            for (_, s) in self.sets.iter().filter(|(_, s)| s.order == order) {
                if s.count == 0 || s.held.handle == 0 {
                    continue;
                }
                let at = match s.anchor {
                    Anchor::At(at) => at,
                    Anchor::On(actor, lift) => match models.actor_ref(actor) {
                        Some(a) if a.visible => [a.at[0], a.at[1], a.at[2] + lift],
                        _ => continue,
                    },
                };
                draw.draw_as(PIPELINE_BADGE, s.held.handle, s.count, 0, DrawList::translation(at, origin), 0.0, 1.0);
            }
            next = orders().filter(|&o| o > order).min();
        }
    }
}
