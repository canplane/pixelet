// pxl-core/sys/io/source.rs

//! The tile source's declaration, made once at boot (pxl:core source
//! `declare`). The source is the published bake's `source` (script/publish.py),
//! or the dev tile server's in the same shape, and the client hands it over
//! whole (ts/client/src/service/sys/fs/source.ts): the zoom the floor is held
//! at, the format and the version each kind is in, and the shards served. A
//! tile that no served shard is over or under is never asked for. The core
//! keeps the declaration for its life and tells it to the UI as it is made and
//! to each UI that attaches (`tell`), whose versions name the tiles' paths.

use pxl_global::Global;
use pxl_spec::bake::publication::{BaseKind, FORMAT};
use pxl_spec::bake::source::{is_sharded, key};
use pxl_spec::space::coord::world::wrap_idx;
use pxl_spec::space::coord::zoom::{Zoom, LEAF_ZOOM, MIN_ZOOM, SHARD_ZOOM};
use pxl_spec::space::layer::area;
use pxl_spec::space::layer::area::Area;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::abi::overlay;
use crate::abi::types::Declaration;

struct Source {
    /// The declaration as it was made, told back to each UI; its zoom is the
    /// one the floor is held at.
    decl: Declaration,
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

/// Declares the source once, and what it says holds while the core runs.
/// `decl` gives each kind's format by its bake's number and its version, and
/// its shards in any order, each with the versions it declares of its own. The
/// declaration is refused whole where the core cannot follow it: a zoom
/// outside the pyramid, a name that is no shard or that names a node over
/// shards (pxl-spec bake/source.rs), a kind with no format or with a format
/// the core does not read, or a version list, the source's or a shard's,
/// without one entry a kind (a shard's entry none where it takes the
/// source's).
pub(crate) fn declare(decl: Declaration) -> Result<(), Errno> {
    let declared = DECLARED_SOURCE.get(|| None);
    if declared.is_some() {
        return Err(Errno::EEXIST);
    }
    *declared = Some(parse(decl).ok_or(Errno::EINVAL)?);
    tell();
    Ok(())
}

/// Tells the UI the declaration, if it is made, at once, so that it reaches a
/// UI before any file opened under it in this entry: as it is made, and to
/// each UI that attaches (link.rs `tell_all`). Its packet is the one the UI
/// declared it in, which fit a frame.
pub(crate) fn tell() {
    if let Some(decl) = decl() {
        overlay::source(decl).or_bug(Errno::EPROTO);
    }
}

fn parse(decl: Declaration) -> Option<Source> {
    let kind_cnt = BaseKind::ALL.len();
    let read = (MIN_ZOOM..=LEAF_ZOOM).contains(&decl.min_zoom)
        && decl.formats.len() == kind_cnt
        && decl.versions.len() == kind_cnt
        && decl.shards.iter().all(|s| s.versions.len() == kind_cnt)
        && BaseKind::ALL
            .into_iter()
            .all(|l| decl.formats[l as usize] == FORMAT[l as usize]);
    if !read {
        return None;
    }
    let shards = decl
        .shards
        .iter()
        .map(|s| key(&s.name))
        .collect::<Option<Vec<_>>>()?;
    Some(Source {
        nine: nines_of(&shards),
        nodes: nodes_of(&shards),
        decl,
    })
}

/// The declaration the core holds, or none before it is made.
pub(crate) fn decl() -> Option<&'static Declaration> {
    declared_source().map(|s| &s.decl)
}

/// Marks each cell at `SHARD_ZOOM` that a served shard covers, then lays the
/// nine of each covered cell and of each cell beside it, once, where its key
/// addresses it (`area::nine`). A row off the world has no key. All such rows
/// count as one cell, which is served where any covered cell is off the
/// world.
fn nines_of(shard: &[Quadrant]) -> Vec<u16> {
    let cells = area::SHARD_CELL_CNT as usize;
    let (mut served, mut off, mut covered) = (vec![false; cells], false, Vec::new());
    for k in shard {
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
fn nodes_of(shard: &[Quadrant]) -> Vec<u8> {
    let mut nodes = vec![0u8; ((1usize << (2 * (SHARD_ZOOM + 1))) - 1) / 3];
    for k in shard {
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
    declared_source().map(|s| s.decl.min_zoom)
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
