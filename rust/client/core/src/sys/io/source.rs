// pxl-core/sys/io/source.rs

//! The tile source's declaration, made once at boot (`SOURCE_DECLARE`): the
//! published bake's `source` (script/publish.py), or the dev tile server's
//! in its shape, of which the client hands over the core's part
//! (ts/client/src/sys/fs/source.ts): the zoom the floor is held at, the
//! format each kind is in, and the shards served, so a tile no served shard
//! is over or under is never asked for. The versions it names are the
//! client's.

use pxl_engine::area::{self, Area};
use pxl_spec::bake::publication::{BaseLayer, FORMAT};
use pxl_spec::bake::source::{is_sharded, key};
use pxl_spec::space::coord::zoom::{Zoom, SHARD_ZOOM};
use pxl_spec::space::layer::quadrant::{Quadrant, LEAF_ZOOM, TOP_ZOOM};
use xpute_core::global::Global;
use xpute_kit::status::errno::Errno;

struct Source {
    top_zoom: Zoom,
    /// The area at `SHARD_ZOOM` (pxl-engine `area::Area`): each cell's nine
    /// by its key (`area::shard_key`), none beside no served one.
    nine: Vec<u16>,
    /// The pyramid's nodes from zoom 0 to `SHARD_ZOOM`, as `node_of` lays
    /// them: whether a shard is at or over each (`OVER`), and at or under it
    /// (`UNDER`).
    nodes: Vec<u8>,
}

const OVER: u8 = 1;
const UNDER: u8 = 2;

static DECLARED_SOURCE: Global<Option<Source>> = Global::new();

fn declared_source() -> Option<&'static Source> {
    DECLARED_SOURCE.get(|| None).as_ref()
}

/// Declared once: what it says holds while the core runs. `format` is each
/// kind's by its bake's number, `shard` the shards' names in any order.
/// Refused whole for what the client cannot follow: a zoom out of the
/// pyramid, a name of no shard or of a node over shards (pxl-spec
/// bake/source.rs), or a kind with no format or one it does not read.
pub fn declare<'a>(
    top_zoom: u32,
    format: &[u32],
    shard: impl IntoIterator<Item = &'a str>,
) -> Result<(), Errno> {
    let declared = DECLARED_SOURCE.get(|| None);
    if declared.is_some() {
        return Err(Errno::EEXIST);
    }
    *declared = Some(parse(top_zoom, format, shard).ok_or(Errno::EINVAL)?);
    Ok(())
}

fn parse<'a>(
    top_zoom: u32,
    format: &[u32],
    names: impl IntoIterator<Item = &'a str>,
) -> Option<Source> {
    let top_zoom = Some(top_zoom).filter(|z| (TOP_ZOOM..=LEAF_ZOOM).contains(z))?;
    let read = format.len() == BaseLayer::ALL.len()
        && BaseLayer::ALL
            .into_iter()
            .all(|l| format[l as usize] == FORMAT.of(l));
    if !read {
        return None;
    }
    let shard = names.into_iter().map(key).collect::<Option<Vec<_>>>()?;
    Some(Source {
        top_zoom,
        nine: nines_of(&shard),
        nodes: nodes_of(&shard),
    })
}

/// Each cell a shard covers at `SHARD_ZOOM` served, then each covered cell's
/// nine, and each beside it, laid once where its key addresses it
/// (`area::nine`). A row off the world has no key: served, as one key for
/// all, where any covered cell is off it.
fn nines_of(shard: &[Quadrant]) -> Vec<u16> {
    let cells = area::SHARD_CELLS as usize;
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
            let n = [
                pxl_engine::wrap_idx(SHARD_ZOOM, x + k % 3 - 1),
                y + k / 3 - 1,
            ];
            let key = area::shard_key(n);
            if key != area::NO_KEY {
                nine[key as usize] = area::nine(serves, n) as u16;
            }
        }
    }
    nine
}

/// A shard is at or under its own node and every node over it, and at or
/// over its own and every node under it, to `SHARD_ZOOM`.
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

/// Where the node `(zoom, at)`, no finer than `SHARD_ZOOM`, lies among the
/// nodes: its zoom's first, then a row of its zoom's south to north
/// (`quad_rows`) and a column round the world. None past the rows.
fn node_of(zoom: Zoom, at: [i32; 2]) -> Option<usize> {
    let n = 1i64 << zoom;
    let row = i64::from(at[1]) + n / 2;
    let first = ((1i64 << (2 * zoom)) - 1) / 3;
    (0..n)
        .contains(&row)
        .then(|| (first + row * n + i64::from(at[0]).rem_euclid(n)) as usize)
}

/// The coarsest zoom the source supplies whole; none until it is declared.
pub fn top() -> Option<Zoom> {
    declared_source().map(|s| s.top_zoom)
}

/// Whether a served shard is over or under `p`, of a kind held by shards.
pub fn serves(layer: BaseLayer, p: Quadrant) -> bool {
    !is_sharded(layer) || declared_source().is_some_and(|s| s.covers(p))
}

/// The area's cells' nines by key, as a GPU's area reads them; none before
/// a source is declared.
pub fn nines() -> &'static [u16] {
    declared_source().map_or(&[], |s| &s.nine)
}

/// The service area as pxl-engine's rules read it: the declared shards, and
/// no end before they are declared.
pub struct ServiceArea;

impl Area for ServiceArea {
    fn ends(&self) -> bool {
        declared_source().is_some()
    }
    fn nine(&self, key: u32) -> u32 {
        declared_source().map_or(0, |s| u32::from(s.nine[key as usize]))
    }
}

impl Source {
    /// A shard over `p`, or at or under it, read off `p`'s node, or past
    /// `SHARD_ZOOM` off the one over it there, which every shard over `p` is
    /// over too.
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
