// pxl-core/domain/query/place.rs

//! The places, as the world's answer. Each place is read from the tile that
//! gives its ground exactly.

use pxl_spec::bake::tile::place::{Place, PlaceTile};
use pxl_spec::space::layer::quadrant::Quadrant;

use super::world::World;
use crate::domain::layer::Base as _;
use crate::domain::layer::tile_cache::Path;

impl World {
    /// Calls `visit` with each place of each of `tiles`, read from the
    /// resident tile that gives the tile's ground exactly, along with that
    /// tile. A tile whose ground no resident tile gives yields nothing.
    /// Returns false where a tile is still to come: asked for, its answer
    /// not settled and nothing to read meanwhile. Each tile's line steps on
    /// from the line of the tile before it, so siblings share the walk.
    pub(crate) fn places_in(
        self,
        tiles: impl IntoIterator<Item = Quadrant>,
        visit: &mut dyn FnMut(Quadrant, Place<'static>),
    ) -> bool {
        let cache: &'static _ = PlaceTile::tile_cache();
        let mut path = Path::default();
        let mut whole = true;
        for tile in tiles {
            let line = cache.seek(&mut path, tile);
            let read = line.src.and_then(|src| Some((src, cache.get(src)?)));
            let Some((src, places)) = read else {
                whole &= line.settled || !cache.is_asked(tile);
                continue;
            };
            for h in places.iter() {
                visit(src, h);
            }
        }
        whole
    }
}
