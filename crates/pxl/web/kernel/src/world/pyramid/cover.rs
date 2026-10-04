// pxl-kernel/world/pyramid/cover.rs

//! What a layer's resident pages draw of the view's pieces: each from the
//! finest resident page at or over it, or a gap. Read by the profiler.
//! Nothing is asked for because of it.

use std::collections::HashMap;

use crate::global::Global;
use crate::io::demand::Category;
use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::domain::{self, Domain};
use crate::world::pyramid::{Layer, Pyramid};

#[derive(Default, PartialEq)]
pub struct Cover {
    pub drawn: Vec<Chunk>,
    pub gaps: Vec<Chunk>,
}

/// A page never waits for its siblings, so ground whose page has come is
/// drawn the turn it lands (Cesium's upsampled tiles).
pub fn cover_of<L: Layer>(d: &Domain, pyramid: &Pyramid<L>) -> Cover {
    let (drawn, gaps) = d.pieces().iter().partition(|c| pyramid.paging().drawn_from(c.page()).is_some());
    Cover { drawn, gaps }
}

#[derive(Default)]
struct Covered {
    key: Option<(u32, u32)>,
    cover: Cover,
    covers: u32,
}

static COVERS: Global<HashMap<Category, Covered>> = Global::new();

fn covered<L: Layer>() -> &'static mut Covered {
    COVERS.get(HashMap::new).entry(L::CATEGORY).or_default()
}

/// Remade only when the pieces or `L`'s residency moved; most eye moves change neither.
pub fn step<L: Layer>() {
    let Some(d) = domain::current() else { return };
    let key = Some((domain::revision(), L::pyramid().paging().revision()));
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

pub fn cover<L: Layer>() -> (&'static Cover, u32) {
    let c = covered::<L>();
    (&c.cover, c.covers)
}

#[cfg(test)]
#[path = "cover.test.rs"]
mod test;
