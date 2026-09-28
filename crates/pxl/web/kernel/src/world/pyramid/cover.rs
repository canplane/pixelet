// pxl-kernel/world/pyramid/cover.rs

//! What a layer's resident pages draw of the view's pieces (domain.rs
//! `Domain`): each piece from the finest resident page at or over it, and
//! the ones nothing over is resident for, as gaps. Made for the frame from
//! the pieces and what is resident, and read by whoever draws a layer — the
//! chunk table for the terrain, the badges for places. Nothing is asked for
//! because of it: what is fetched is the observation's demand
//! (world/pyramid.rs).

use std::collections::HashMap;

use crate::global::Global;
use crate::io::demand::Category;
use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::domain::{self, Domain};
use crate::world::pyramid::{Layer, Pyramid};

/// What the resident pages draw of a view's `Domain`: the chunks the rule
/// wants, each drawn from the finest resident page at or over it, and the
/// ones nothing over is resident for, as gaps — the two together being the
/// domain's ground exactly, once. Whatever is counted over the cover is
/// counted once, since every piece is a chunk the rule chose and none lies
/// over another.
#[derive(Default, PartialEq)]
pub struct Cover {
    pub drawn: Vec<Chunk>,
    pub gaps: Vec<Chunk>,
}

/// What the resident pages of `pyramid` draw of `d`: every piece in its own
/// place, drawn from its page or the nearest resident page over it
/// (`drawn_from`), and a gap where nothing over it is resident. A page is
/// never waited on for its siblings, so ground whose own page has come is
/// drawn at once however little of the ground around it has, and whatever
/// arrives first is on the screen the turn it lands (Cesium's upsampled
/// tiles).
pub fn cover_of<L: Layer>(d: &Domain, pyramid: &Pyramid<L>) -> Cover {
    let (drawn, gaps) = d.pieces().iter().partition(|c| pyramid.drawn_from(c.page()).is_some());
    Cover { drawn, gaps }
}

/// A layer's cover as last made, what it was made from, and which making of
/// it this is.
#[derive(Default)]
struct Covered {
    key: Option<(u32, u32)>,
    cover: Cover,
    covers: u32,
}

/// Every layer's, by the stream its pages are asked under.
static COVERS: Global<HashMap<Category, Covered>> = Global::new();

fn covered<L: Layer>() -> &'static mut Covered {
    COVERS.get(HashMap::new).entry(L::CATEGORY).or_default()
}

/// Makes `L`'s cover again wherever the pieces or what `L` holds have moved.
/// The eye moves every frame the camera does, and most of those moves cross
/// no chunk's split distance: whoever reads the cover is told only when what
/// it draws has changed.
pub fn step<L: Layer>() {
    domain::step();
    let Some(d) = domain::current() else { return };
    let key = Some((domain::revision(), L::pyramid().revision()));
    let c = covered::<L>();
    if c.key == key {
        return;
    }
    c.key = key;
    let cover = cover_of(d, L::pyramid());
    if cover != c.cover {
        c.cover = cover;
        c.covers = c.covers.wrapping_add(1);
    }
}

/// What `L` draws of the view, and which making of it this is.
pub fn cover<L: Layer>() -> (&'static Cover, u32) {
    let c = covered::<L>();
    (&c.cover, c.covers)
}

#[cfg(test)]
#[path = "cover.test.rs"]
mod test;
