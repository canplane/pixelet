// pxl-spec/bake/source.rs

//! What a tile source's declaration may name, held alike where it is written
//! (pxl-geo-bake source.rs) and where it is read (pxl-core sys/io/source.rs):
//! a declaration one side writes is one the other reads.

use crate::bake::publication::BaseKind;
use crate::space::coord::zoom::SHARD_ZOOM;
use crate::space::layer::quadrant::Quadrant;

use crate::bake::publication::{VERSION_CHARS, VERSION_SZ};

/// A key of its shards: a shard, or a node over shards.
pub fn key(name: &str) -> Option<Quadrant> {
    Quadrant::of_key(name).filter(|t| t.zoom <= SHARD_ZOOM)
}

/// A version is one segment of a URL, so a tile's name stays under it. The client's source
/// holds the same rule (ts/client/src/service/sys/fs/source.ts), and both are held to one record
/// (spec/golden/bake/version.tsv).
pub fn is_version(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= VERSION_SZ
        && version
            .bytes()
            .all(|b| VERSION_CHARS.as_bytes().contains(&b))
}

/// Whether a kind is held a shard at a time: the pyramid's kinds are; the imagery,
/// the global image's, belongs to no shard.
pub fn is_sharded(layer: BaseKind) -> bool {
    layer != BaseKind::Imagery
}

#[cfg(test)]
#[path = "source.test.rs"]
mod test;
