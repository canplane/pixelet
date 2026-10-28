// pxl-spec/bake/source.rs

//! What a tile source's declaration may name, held alike where it is written
//! (pxl-geo-bake source.rs) and where it is read (pxl-kernel sys/io/source.rs):
//! a declaration one side writes is one the other reads.

use crate::bake::published::BaseLayer;
use crate::space::coord::zoom::SHARD_ZOOM;
use crate::space::layer::tile::Tile;

use crate::bake::published::VERSION_BYTES;

/// A key of its shards: a shard, or a node over shards.
pub fn key(name: &str) -> Option<Tile> {
    Tile::of_name(name).filter(|t| t.zoom <= SHARD_ZOOM)
}

/// A version is one segment of a URL, so a tile's name stays under it.
pub fn is_version(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= VERSION_BYTES
        && version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

/// Whether a kind is held a shard at a time: the pyramid's are, the Earth's
/// picture, the sphere's, is no shard's.
pub fn sharded(layer: BaseLayer) -> bool {
    layer != BaseLayer::earth
}
