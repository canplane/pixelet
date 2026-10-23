// pxl-kernel/io/source.rs

//! The tile source's declaration, read once at boot: the published bake's
//! `source` out of the bucket (script/publish.ts), or the dev tile server's
//! answer in its shape. It says the zoom the floor is held at, the
//! shards served, the format each kind is in and where each kind is: a tile
//! no served shard is over or under is never asked for, and a tile is opened
//! under the version that serves it, which the client does not read.

use pxl_portable::area::{self, Area};
use pxl_spec::bake::published::{BaseLayer, FORMAT};
use pxl_spec::bake::source::{is_version, key, sharded};
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::Cells;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};
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
    area::past(&Declared, leaf.map(|v| v as i32))
}

/// The service area as pxl-portable's rules read it: the declared source's
/// shards, and no end before one is declared.
pub struct Declared;

impl Area for Declared {
    fn ends(&self) -> bool {
        declared().is_some()
    }
    fn shards(&self) -> u32 {
        declared().map_or(0, |s| s.shard.len() as u32)
    }
    fn shard(&self, i: u32) -> (Zoom, [i32; 2]) {
        let t = declared().or_bug(Errno::ENOTRECOVERABLE).shard[i as usize].tile;
        (t.zoom, t.at.map(|v| v as i32))
    }
}

/// Each served shard's tile, as a GPU's area reads it.
pub fn served(each: &mut dyn FnMut(Zoom, [i32; 2])) {
    if let Some(s) = declared() {
        for k in &s.shard {
            each(k.tile.zoom, k.tile.at.map(|v| v as i32));
        }
    }
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
    fn covers(&self, p: Tile) -> bool {
        self.shard.iter().any(|k| over(k.tile, p) || over(p, k.tile))
    }

    /// The finest key over `p` that names one for `layer`, else the source's.
    fn version_of(&self, layer: BaseLayer, p: Tile) -> &str {
        let own = self
            .shard
            .iter()
            .filter(|k| sharded(layer) && over(k.tile, p))
            .filter_map(|k| Some((k.tile.zoom, k.version[layer as usize].as_deref()?)))
            .max_by_key(|&(zoom, _)| zoom);
        own.map_or(&self.version[layer as usize], |(_, v)| v)
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
    Some(Source {
        top_zoom,
        version: version.map(Option::unwrap_or_default),
        shard,
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
