// pxl-kernel/sys/io/source.rs

//! The tile source's declaration, read once at boot: the published bake's
//! `source` out of the bucket (apps/web/script/publish.ts), or the dev tile server's
//! answer in its shape. The client reads its text and hands over what is the
//! kernel's (apps/client/src/namespace/source.ts, `WORLD_SOURCE`): the zoom
//! the floor is held at, the format each kind is in, and the shards served,
//! so a tile no served shard is over or under is never asked for. The
//! versions it names are the client's, which reads a tile under the one that
//! serves it.

use pxl_spec::bake::published::{BaseLayer, FORMAT};
use pxl_spec::bake::source::{key, sharded};
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::zoom::{Zoom, SHARD_ZOOM};
use pxl_spec::space::layer::lattice::Cells;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM, TOP_ZOOM};
use pxl_spec_core::area::{self, Area};
use xpute_guest::global::Global;
use xpute_kit::status::errno::Errno;

struct Source {
    top_zoom: Zoom,
    /// The area at `SHARD_ZOOM` (pxl-spec-core `area::Area`): every cell's
    /// nine by its key (`area::shard_key`), none beside no served one.
    nine: Vec<u16>,
    /// The pyramid's nodes from zoom 0 to `SHARD_ZOOM`, each where its zoom
    /// and its row and column address it (`node_of`): the finest shard over
    /// it, one past its index (0 none), and whether a shard is at or under
    /// it (`UNDER`).
    nodes: Vec<u32>,
}

/// A node's word's bit: a shard is at or under it.
const UNDER: u32 = 1 << 16;

/// Where the node `(zoom, at)`, no finer than `SHARD_ZOOM`, lies among
/// `Source::nodes`: its zoom's first, then a row of its zoom's tiles south
/// to north (`tile_rows`) and a column round the world. None past the rows.
fn node_of(zoom: Zoom, at: [i32; 2]) -> Option<usize> {
    let n = 1i64 << zoom;
    let row = i64::from(at[1]) + n / 2;
    let first = ((1i64 << (2 * zoom)) - 1) / 3;
    (0..n).contains(&row).then(|| (first + row * n + i64::from(at[0]).rem_euclid(n)) as usize)
}

static DECLARED: Global<Option<Source>> = Global::new();

/// Declared once, at boot: what it says is read as long as the kernel runs.
/// `format` is each kind's by its bake's number, and `shard` the shards'
/// names, in any order. Refused whole for anything the client cannot follow:
/// a zoom out of the pyramid, a name no shard or node over shards (pxl-spec
/// bake/source.rs), a kind with no format or one it does not read, or more
/// shards than a node's 16 bits can index.
pub fn declare<'a>(top_zoom: u32, format: &[u32], shard: impl IntoIterator<Item = &'a str>) -> Result<(), Errno> {
    let declared = DECLARED.get(Default::default);
    if declared.is_some() {
        return Err(Errno::EEXIST);
    }
    *declared = Some(parse(top_zoom, format, shard).ok_or(Errno::EINVAL)?);
    Ok(())
}

fn declared() -> Option<&'static Source> {
    DECLARED.get(Default::default).as_ref()
}

/// The coarsest zoom the source supplies whole; none until it is declared.
pub fn top() -> Option<Zoom> {
    declared().map(|s| s.top_zoom)
}

/// Whether a served shard is over or under `p`, of a kind held by shards.
pub fn serves(layer: BaseLayer, p: Tile) -> bool {
    !sharded(layer) || declared().is_some_and(|s| s.covers(p))
}

/// Whether the ground at `[x, y]`, in units, is past the service area: the
/// source is declared and serves no shard over its leaf. The one judgement of
/// where the area ends, for what is drawn, walked, edited and said alike.
pub fn past_at([x, y]: [f64; 2]) -> bool {
    let leaf = GROUND.cell_of(LEAF_ZOOM, &world::point(x, y, 0.0)).tile().at;
    area::past(&Declared, leaf)
}

/// The service area as pxl-spec-core's rules read it: the declared source's
/// shards, and no end before one is declared.
pub struct Declared;

impl Area for Declared {
    fn ends(&self) -> bool {
        declared().is_some()
    }
    fn nine(&self, key: u32) -> u32 {
        declared().map_or(0, |s| u32::from(s.nine[key as usize]))
    }
}

/// The area's cells' nines by key, as a GPU's area reads them (`Area`);
/// none before a source is declared.
pub fn nines() -> &'static [u16] {
    declared().map_or(&[], |s| &s.nine)
}

impl Source {
    /// `p`'s node among `nodes`: itself, or past `SHARD_ZOOM` the one over
    /// it there, which every shard over it is over too (no shard is finer).
    fn node_over(&self, p: Tile) -> Option<u32> {
        let z = p.zoom.min(SHARD_ZOOM);
        node_of(z, Tile::over(z, p.first_leaf()).at).map(|i| self.nodes[i])
    }

    /// A shard over `p`, or at or under it: one read.
    fn covers(&self, p: Tile) -> bool {
        self.node_over(p).is_some_and(|w| w & 0xffff != 0 || p.zoom <= SHARD_ZOOM && w & UNDER != 0)
    }
}

fn parse<'a>(top_zoom: u32, format: &[u32], names: impl IntoIterator<Item = &'a str>) -> Option<Source> {
    let top_zoom: Zoom = Some(top_zoom).filter(|z| (TOP_ZOOM..=LEAF_ZOOM).contains(z))?;
    reads_every(format)?;
    let mut shard = Vec::new();
    for name in names {
        shard.push(key(name)?);
    }
    // A node's word holds one past a shard's index in 16 bits.
    (shard.len() <= 0xffff).then_some(())?;
    // Every cell a shard covers at `SHARD_ZOOM`, a key no finer (`key`),
    // served; then every cell it and each beside it, its nine laid once
    // where its key addresses it (`area::nine`). A row off the world has no
    // key: served where any covered cell is off it, as one key for all.
    let cells = area::SHARD_CELLS as usize;
    let (mut served, mut off, mut covered) = (vec![false; cells], false, Vec::new());
    for k in &shard {
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
            let n = [pxl_spec_core::wrap_index(SHARD_ZOOM, x + k % 3 - 1), y + k / 3 - 1];
            let key = area::shard_key(n);
            if key != area::NO_KEY {
                nine[key as usize] = area::nine(serves, n) as u16;
            }
        }
    }
    // The pyramid's nodes to `SHARD_ZOOM`: a shard is at or under its own
    // node and every node over it, and the finest shard over each node at
    // or under its own; no two at a zoom are over one node.
    let mut nodes = vec![0u32; ((1usize << (2 * (SHARD_ZOOM + 1))) - 1) / 3];
    for (n, k) in shard.iter().enumerate() {
        let (zk, at) = (k.zoom, k.at);
        for z in 0..=zk {
            if let Some(i) = node_of(z, at.map(|v| v >> (zk - z))) {
                nodes[i] |= UNDER;
            }
        }
        for z in zk..=SHARD_ZOOM {
            let d = z - zk;
            for j in 0..1 << d {
                for i in 0..1 << d {
                    let Some(at) = node_of(z, [(at[0] << d) + i, (at[1] << d) + j]) else { continue };
                    let was = nodes[at] & 0xffff;
                    if was == 0 || shard[was as usize - 1].zoom <= zk {
                        nodes[at] = nodes[at] & UNDER | (n as u32 + 1);
                    }
                }
            }
        }
    }
    Some(Source { top_zoom, nine, nodes })
}

/// Every kind is in the format this client reads it in, by its bake's number.
fn reads_every(format: &[u32]) -> Option<()> {
    (format.len() == BaseLayer::ALL.len() && BaseLayer::ALL.into_iter().all(|l| format[l as usize] == FORMAT.of(l))).then_some(())
}

#[cfg(test)]
#[path = "source.test.rs"]
mod test;
