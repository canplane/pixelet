// pxl-core/sys/io/source.rs

//! The tile source's declaration, made by each UI at its boot (pxl:core source
//! `declare`). The source is the published bake's `source` (script/publish.py),
//! or the dev tile server's in the same shape, and the client hands over the
//! core's part of it (ts/client/src/service/sys/fs/source.ts): the zoom the
//! floor is held at, the format each kind is in, and the shards served. A tile
//! that no served shard is over or under is never asked for. The versions the
//! source names stay with the client.

use pxl_global::Global;
use pxl_spec::bake::publication::{BaseKind, FORMAT};
use pxl_spec::bake::source::{is_sharded, key};
use pxl_spec::space::coord::world::wrap_idx;
use pxl_spec::space::coord::zoom::{LEAF_ZOOM, MIN_ZOOM, SHARD_ZOOM, Zoom};
use pxl_spec::space::layer::area;
use pxl_spec::space::layer::area::Area;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_kit::status::errno::Errno;

#[derive(PartialEq)]
struct Source {
    min_zoom: Zoom,
    /// The service area at `SHARD_ZOOM` (pxl-spec `area::Area`). For each
    /// cell, by its key (`area::shard_key`), it holds the bits of which of the
    /// nine cells about it are served. A cell beside no served cell holds 0.
    nine: Vec<u16>,
    /// The pyramid's nodes from zoom 0 to `SHARD_ZOOM`, placed as `node_of`
    /// places them. A node holds `OVER` where a served shard is at or over it,
    /// and `UNDER` where one is at or under it.
    nodes: Vec<u8>,
}

const OVER: u8 = 1;
const UNDER: u8 = 2;

static DECLARED_SOURCE: Global<Option<Source>> = Global::new();

fn declared_source() -> Option<&'static Source> {
    DECLARED_SOURCE.get(|| None).as_ref()
}

/// Declares the source, and what it says holds while the core runs.
/// `formats` gives each kind's format by its bake's number, and `shards` the
/// shards' names in any order. The declaration is refused whole with EINVAL
/// where the core cannot follow it: a zoom outside the pyramid, a name that is
/// no shard or that names a node over shards (pxl-spec bake/source.rs), or a
/// kind with no format or with a format the core does not read. Once one is
/// held, a declaration that parses to it changes nothing, as a UI that
/// attaches to a running core makes, and any other is refused with EEXIST.
pub(crate) fn declare<'a>(
    min_zoom: u32,
    formats: &[u32],
    shards: impl IntoIterator<Item = &'a str>,
) -> Result<(), Errno> {
    let source = parse(min_zoom, formats, shards).ok_or(Errno::EINVAL)?;
    let declared = DECLARED_SOURCE.get(|| None);
    match declared {
        None => *declared = Some(source),
        Some(cur) if *cur != source => return Err(Errno::EEXIST),
        Some(_) => {}
    }
    Ok(())
}

fn parse<'a>(
    min_zoom: u32,
    formats: &[u32],
    shards: impl IntoIterator<Item = &'a str>,
) -> Option<Source> {
    let min_zoom = Some(min_zoom).filter(|z| (MIN_ZOOM..=LEAF_ZOOM).contains(z))?;
    let read = formats.len() == BaseKind::ALL.len()
        && BaseKind::ALL
            .into_iter()
            .all(|l| formats[l as usize] == FORMAT[l as usize]);
    if !read {
        return None;
    }
    let shards = shards.into_iter().map(key).collect::<Option<Vec<_>>>()?;
    Some(Source {
        min_zoom,
        nine: nines_of(&shards),
        nodes: nodes_of(&shards),
    })
}

/// Marks each cell at `SHARD_ZOOM` that a served shard covers, then lays the
/// nine of each covered cell and of each cell beside it, once, where its key
/// addresses it (`area::nine`). A row off the world has no key. All such rows
/// count as one cell, which is served where any covered cell is off the
/// world.
fn nines_of(shards: &[Quadrant]) -> Vec<u16> {
    let cells = area::SHARD_CELL_CNT as usize;
    let (mut served, mut off, mut covered) = (vec![false; cells], false, Vec::new());
    for k in shards {
        let d = SHARD_ZOOM - k.zoom;
        let [x, y] = k.at.map(|v| v << d);
        for j in 0..1 << d {
            for i in 0..1 << d {
                match area::shard_key([x + i, y + j]) {
                    area::NO_KEY => off = true,
                    key => served[key as usize] = true,
                }
                covered.push([x + i, y + j]);
            }
        }
    }
    let serves = |c: [i32; 2]| match area::shard_key(c) {
        area::NO_KEY => off,
        key => served[key as usize],
    };
    let mut nine = vec![0u16; cells];
    for &[x, y] in &covered {
        for k in 0..9 {
            let n = [wrap_idx(SHARD_ZOOM, x + k % 3 - 1), y + k / 3 - 1];
            let key = area::shard_key(n);
            if key != area::NO_KEY {
                nine[key as usize] = area::nine(serves, n) as u16;
            }
        }
    }
    nine
}

/// Marks the nodes for each shard. A shard is at or under its own node and
/// every node over it, and at or over its own node and every node under it,
/// down to `SHARD_ZOOM`.
fn nodes_of(shards: &[Quadrant]) -> Vec<u8> {
    let mut nodes = vec![0u8; ((1usize << (2 * (SHARD_ZOOM + 1))) - 1) / 3];
    for k in shards {
        let (zk, at) = (k.zoom, k.at);
        for z in 0..=zk {
            if let Some(n) = node_of(z, at.map(|v| v >> (zk - z))) {
                nodes[n] |= UNDER;
            }
        }
        for z in zk..=SHARD_ZOOM {
            let d = z - zk;
            for j in 0..1 << d {
                for i in 0..1 << d {
                    if let Some(n) = node_of(z, [(at[0] << d) + i, (at[1] << d) + j]) {
                        nodes[n] |= OVER;
                    }
                }
            }
        }
    }
    nodes
}

/// The index of the node `(zoom, at)`, no finer than `SHARD_ZOOM`, among the
/// nodes. The nodes of a zoom follow those of every coarser zoom. Within a
/// zoom they lie by row from south to north (`quad_rows`), then by column
/// around the world. A row off the world has no index.
fn node_of(zoom: Zoom, at: [i32; 2]) -> Option<usize> {
    let n = 1i64 << zoom;
    let row = i64::from(at[1]) + n / 2;
    let first = ((1i64 << (2 * zoom)) - 1) / 3;
    (0..n)
        .contains(&row)
        .then(|| (first + row * n + i64::from(at[0]).rem_euclid(n)) as usize)
}

/// The coarsest zoom the source supplies whole; none until it is declared.
pub(crate) fn min_zoom() -> Option<Zoom> {
    declared_source().map(|s| s.min_zoom)
}

/// Whether a served shard is over or under `p`. A kind that shards do not
/// hold is served everywhere.
pub(crate) fn serves(layer: BaseKind, p: Quadrant) -> bool {
    !is_sharded(layer) || declared_source().is_some_and(|s| s.covers(p))
}

/// The nine of each cell of the area, by key, as a GPU's area reads them.
/// It is empty before a source is declared.
pub(crate) fn nines() -> &'static [u16] {
    declared_source().map_or(&[], |s| &s.nine)
}

/// The service area as pxl-engine's rules read it: the declared shards.
/// Before they are declared, the area has no end.
pub(crate) struct ServiceArea;

impl Area for ServiceArea {
    fn ends(&self) -> bool {
        declared_source().is_some()
    }
    fn nine(&self, key: u32) -> u32 {
        declared_source().map_or(0, |s| u32::from(s.nine[key as usize]))
    }
}

impl Source {
    /// Whether a served shard is over `p`, or at or under it. The answer is
    /// read off `p`'s node. Finer than `SHARD_ZOOM`, it is read off the node
    /// at `SHARD_ZOOM` over `p`, because every shard over `p` is over that
    /// node too.
    fn covers(&self, p: Quadrant) -> bool {
        let (z, at) = area::shard_over(p.zoom, p.at);
        node_of(z, at).is_some_and(|n| {
            let w = self.nodes[n];
            w & OVER != 0 || p.zoom <= SHARD_ZOOM && w & UNDER != 0
        })
    }
}

#[cfg(test)]
#[path = "source.test.rs"]
mod test;
