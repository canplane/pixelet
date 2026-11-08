// pxl-core/domain/query/place.rs

//! The places, as the world's answer. Each place is read from the tile that
//! gives its ground exactly, and is named by that tile and its index there.
//! The name reads the place again for as long as the tile is held.

use pxl_spec::bake::tile::place::{PlaceTile, TilePlace};
use pxl_spec::space::layer::quadrant::Quadrant;

use super::world::World;
use crate::domain::layer::tile_cache::Lines;
use crate::domain::layer::Base as _;

impl World<'static> {
    /// Calls `visit` with each place of each of `tiles`, read from the
    /// resident tile that gives the tile's ground exactly, along with that
    /// tile and the place's index in it. A tile whose ground no resident tile
    /// gives yields nothing. Each tile's line steps on from the line of the
    /// tile before it, so siblings share the walk.
    pub fn places_in(
        self,
        tiles: impl IntoIterator<Item = Quadrant>,
        visit: &mut dyn FnMut(Quadrant, u32, TilePlace<'static>),
    ) {
        let held: &'static _ = PlaceTile::tile_cache();
        let mut lines = Lines::default();
        for tile in tiles {
            let Some(src) = held.seek(&mut lines, tile).src else {
                continue;
            };
            let Some(places) = held.get(src) else {
                continue;
            };
            for (place, h) in places.iter() {
                visit(src, place, h);
            }
        }
    }
}
