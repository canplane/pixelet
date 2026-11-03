// pxl-spec/bake/source.rs

//! What a tile source's declaration may name, held alike where it is written
//! (pxl-geo-bake source.rs) and where it is read (pxl-kernel sys/io/source.rs):
//! a declaration one side writes is one the other reads.

use crate::bake::publication::BaseLayer;
use crate::space::coord::zoom::SHARD_ZOOM;
use crate::space::layer::quadrant::Quadrant;

use crate::bake::publication::{VERSION_BYTES, VERSION_CHARS};

/// A key of its shards: a shard, or a node over shards.
pub fn key(name: &str) -> Option<Quadrant> {
    Quadrant::of_key(name).filter(|t| t.zoom <= SHARD_ZOOM)
}

/// A version is one segment of a URL, so a tile's name stays under it.
pub fn is_version(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= VERSION_BYTES
        && version
            .bytes()
            .all(|b| VERSION_CHARS.as_bytes().contains(&b))
}

/// Whether a kind is held a shard at a time: the pyramid's are, the Earth's
/// picture, the sphere's, is no shard's.
pub fn is_sharded(layer: BaseLayer) -> bool {
    layer != BaseLayer::earth
}
