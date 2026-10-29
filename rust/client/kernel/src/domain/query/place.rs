// pxl-kernel/domain/query/place.rs

//! What the places say, as the world's answer: each place read from the
//! tile that says its ground exactly, and named by that tile and its index
//! there, which reads it again while the tile is held.

use pxl_spec::bake::tile::place::{PlaceTile, Placed};
use pxl_spec::space::layer::tile::Tile;

use super::world::World;
use crate::domain::layer::tile_cache::Lines;
use crate::domain::layer::Base as _;

impl World<'static> {
    /// Each place of each of `tiles`, from the resident tile that says its
    /// ground exactly, with that tile and the place's index in it; nothing
    /// for a tile none says. Siblings' lines step from one another's.
    pub fn places_in(
        self,
        tiles: impl IntoIterator<Item = Tile>,
        visit: &mut dyn FnMut(Tile, u32, Placed<'static>),
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

    /// The place `place` of `tile`, as `places_in` named it; none once that
    /// tile has gone, where another tile's index names another place.
    pub fn place(self, tile: Tile, place: u32) -> Option<Placed<'static>> {
        let placed: &'static PlaceTile = PlaceTile::tile_cache().get(tile)?;
        (place < placed.len()).then(|| placed.get(place))
    }
}
