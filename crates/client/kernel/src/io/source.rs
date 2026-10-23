// pxl-kernel/io/source.rs

//! The tile source's declaration, read once at boot: the published bake's
//! `source` out of the bucket (script/publish.ts), or the dev tile server's
//! answer in its shape. It says the zoom the floor is held at, the
//! shards served, the format each kind is in and where each kind is: a tile
//! no served shard is over or under is never asked for, and a tile is opened
//! under the version that serves it, which the client does not read.

use pxl_spec::bake::published::{BaseLayer, FORMAT};
use pxl_spec::bake::source::{is_version, key, sharded};
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::zoom::{Zoom, SHARD_ZOOM};
use pxl_spec::space::layer::lattice::Cells;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};
use pxl_spec_core::area::{self, Area};
use xpute_guest::global::Global;
use xpute_kit::codec::json::{read_json, JsonType, JsonValue};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

/// A kind's version each, where one is named.
type Versions = [Option<String>; BaseLayer::ALL.len()];

struct Shard {
    tile: Tile,
    version: Versions,
}

struct Source {
    top_zoom: Zoom,
    version: [String; BaseLayer::ALL.len()],
    shard: Vec<Shard>,
    /// The area at `SHARD_ZOOM` (pxl-spec-core `area::Area`): every cell's
    /// nine by its key (`area::shard_key`), none beside no served one.
    nine: Vec<u16>,
    /// The pyramid's nodes from zoom 0 to `SHARD_ZOOM`, each where its zoom
    /// and its row and column address it (`node_of`): the finest shard over
    /// it, one past its index (0 none), and whether a shard is at or under
    /// it (`UNDER`).
    nodes: Vec<u32>,
    /// Each shard's version a kind, as the finest shard over it, itself
    /// among them, that names one, the last of the finest as declared
    /// (`version_of`); none where none does.
    versions: Vec<[Option<u16>; BaseLayer::ALL.len()]>,
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
/// Refused whole for anything the client cannot follow: a zoom out of the
/// pyramid, a key no shard or node over shards (pxl-spec bake/source.rs), a
/// kind with no format or one it does not read, or with no version, a
/// version that is no URL segment.
pub fn declare(text: &str) -> Result<(), Errno> {
    let declared = DECLARED.get(Default::default);
    if declared.is_some() {
        return Err(Errno::EEXIST);
    }
    *declared = Some(parse(text).ok_or(Errno::EINVAL)?);
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

/// The kind's own version, over every shard's: an object no shard holds
/// (`pxl-spec route` `TILE_SPHERE`) is opened under it.
pub fn whole(layer: BaseLayer) -> &'static str {
    &declared().or_bug(Errno::ENOTRECOVERABLE).version[layer as usize]
}

/// The version `p` is opened under. No tile is asked for before the source
/// is declared: nothing is observed until then (view/observation.rs).
pub fn version(layer: BaseLayer, p: Tile) -> &'static str {
    declared().or_bug(Errno::ENOTRECOVERABLE).version_of(layer, p)
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

    /// The finest key over `p` that names one for `layer`, else the source's:
    /// the finest shard over it, then its version's (`versions`).
    fn version_of(&self, layer: BaseLayer, p: Tile) -> &str {
        let finest = self.node_over(p).map_or(0, |w| w & 0xffff);
        let own = (sharded(layer) && finest != 0).then(|| self.versions[finest as usize - 1][layer as usize]).flatten();
        own.map_or(&self.version[layer as usize], |k| {
            self.shard[usize::from(k)].version[layer as usize].as_deref().or_bug(Errno::ENOTRECOVERABLE)
        })
    }
}

/// Each kind's version as declared, the one over every shard's own; none
/// until declared.
pub fn versions(each: &mut dyn FnMut(BaseLayer, &str)) {
    if let Some(s) = declared() {
        for l in BaseLayer::ALL {
            each(l, &s.version[l as usize]);
        }
    }
}

/// `k` is `p` or a tile over it.
fn over(k: Tile, p: Tile) -> bool {
    k.zoom <= p.zoom && Tile::over(k.zoom, p.first_leaf()) == k
}

fn parse(text: &str) -> Option<Source> {
    let o = read_json(text).filter(|v| v.type_of() == JsonType::Object)?;
    let pyramid = f64::from(GROUND.top())..=f64::from(LEAF_ZOOM);
    let top_zoom = o.get("top_zoom")?.as_f64().filter(|z| z.fract() == 0.0 && pyramid.contains(z))? as Zoom;
    read_format(o.get("format")?)?;
    let mut shard = Vec::new();
    for (name, entry) in o.get("shard").filter(|v| v.type_of() == JsonType::Object)?.fields() {
        shard.push(Shard {
            tile: key(name.as_plain()?)?,
            version: read_versions(entry.get("version"))?,
        });
    }
    let version = read_versions(o.get("version"))?;
    if version.iter().any(Option::is_none) {
        return None;
    }
    // Every cell a shard covers at `SHARD_ZOOM`, a key no finer (`key`),
    // served; then every cell it and each beside it, its nine laid once
    // where its key addresses it (`area::nine`). A row off the world has no
    // key: served where any covered cell is off it, as one key for all.
    let cells = area::SHARD_CELLS as usize;
    let (mut served, mut off, mut covered) = (vec![false; cells], false, Vec::new());
    for k in &shard {
        let d = SHARD_ZOOM - k.tile.zoom;
        let [x, y] = k.tile.at.map(|v| v << d);
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
    // or under its own, the last of the finest as declared.
    let mut nodes = vec![0u32; ((1usize << (2 * (SHARD_ZOOM + 1))) - 1) / 3];
    for (n, k) in shard.iter().enumerate() {
        let (zk, at) = (k.tile.zoom, k.tile.at);
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
                    if was == 0 || shard[was as usize - 1].tile.zoom <= zk {
                        nodes[at] = nodes[at] & UNDER | (n as u32 + 1);
                    }
                }
            }
        }
    }
    // Each shard's version a kind: the finest shard over it that names one,
    // the last of the finest as declared, as `max_by_key` keeps the last of
    // equals.
    let versions = (0..shard.len())
        .map(|i| {
            core::array::from_fn(|l| {
                let mut best: Option<usize> = None;
                for (j, k) in shard.iter().enumerate() {
                    if over(k.tile, shard[i].tile) && k.version[l].is_some() && best.is_none_or(|b| shard[b].tile.zoom <= k.tile.zoom) {
                        best = Some(j);
                    }
                }
                best.map(|b| b as u16)
            })
        })
        .collect();
    Some(Source {
        top_zoom,
        version: version.map(Option::unwrap_or_default),
        shard,
        nine,
        nodes,
        versions,
    })
}

/// Every kind is in the format this client reads it in.
fn read_format(v: JsonValue<'_>) -> Option<()> {
    let reads = |l: BaseLayer| v.get(l.name())?.as_f64().filter(|&n| n == f64::from(FORMAT.of(l)));
    (v.type_of() == JsonType::Object && BaseLayer::ALL.into_iter().all(|l| reads(l).is_some())).then_some(())
}

fn read_versions(v: Option<JsonValue<'_>>) -> Option<Versions> {
    let mut out = Versions::default();
    let Some(v) = v else { return Some(out) };
    if v.type_of() != JsonType::Object {
        return None;
    }
    for (kind, version) in v.fields() {
        let layer = BaseLayer::ALL.into_iter().find(|l| kind.eq_str(l.name()))?;
        let version = version.as_str()?.as_plain()?;
        out[layer as usize] = Some(is_version(version).then(|| version.to_string())?);
    }
    Some(out)
}

#[cfg(test)]
#[path = "source.test.rs"]
mod test;
