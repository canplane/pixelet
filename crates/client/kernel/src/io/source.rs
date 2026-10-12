// pxl-kernel/io/source.rs

//! The tile source's declaration, read once at boot: the published bake's
//! `source` out of the bucket, or the dev tile server's answer in its shape
//! (published.json's `SOURCE`). It says the zoom the floor is held at, the
//! shards served, the format each kind is in and where each kind is: a tile
//! no served shard is over or under is never asked for, and a tile is opened
//! under the path that serves it, which the client does not read.

use pxl_spec::bake::published::{BaseLayer, FORMAT};
use pxl_spec::space::coord::zoom::{Zoom, SHARD_ZOOM};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};
use xpute_guest::global::Global;
use xpute_kit::codec::json::{read_json, JsonType, JsonValue};
use xpute_kit::status::errno::Errno;

/// The longest path a source names before a tile's name, so a path the
/// kernel opens is bounded (io/file.rs `PATH_BYTES`); a published version's
/// name is 19 at most.
pub const SOURCE_PATH_BYTES: usize = 32;

type Paths = [Option<String>; BaseLayer::ALL.len()];

struct Shard {
    tile: Tile,
    path: Paths,
}

struct Source {
    top_zoom: Zoom,
    path: Paths,
    shard: Vec<Shard>,
}

static DECLARED: Global<Option<Source>> = Global::new();

/// Refused whole for anything the client cannot follow: a zoom out of the
/// pyramid, a key no shard or node over shards, a format it does not read, a
/// path that is no single segment.
pub fn declare(text: &str) -> Result<(), Errno> {
    *DECLARED.get(Default::default) = Some(parse(text).ok_or(Errno::EINVAL)?);
    Ok(())
}

fn declared() -> Option<&'static Source> {
    DECLARED.get(Default::default).as_ref()
}

/// The coarsest zoom the source supplies whole; none until it is declared.
pub fn top() -> Option<Zoom> {
    declared().map(|s| s.top_zoom)
}

/// Whether a served shard is over or under `p`. The Earth's picture is the
/// sphere's, which no shard divides.
pub fn serves(layer: BaseLayer, p: Tile) -> bool {
    layer == BaseLayer::earth || declared().is_some_and(|s| s.shard.iter().any(|k| over(k.tile, p) || over(p, k.tile)))
}

/// The path `p` is opened under: the finest key over it that names one for
/// `layer`, else the source's; none from a source that names none.
pub fn path(layer: BaseLayer, p: Tile) -> Option<&'static str> {
    let s = declared()?;
    let own = s
        .shard
        .iter()
        .filter(|k| layer != BaseLayer::earth && over(k.tile, p))
        .filter_map(|k| Some((k.tile.zoom, k.path[layer as usize].as_deref()?)))
        .max_by_key(|&(zoom, _)| zoom);
    own.map(|(_, v)| v).or(s.path[layer as usize].as_deref())
}

/// `k` is `p` or a tile over it.
fn over(k: Tile, p: Tile) -> bool {
    k.zoom <= p.zoom && Tile::over(k.zoom, p.first_leaf()) == k
}

fn parse(text: &str) -> Option<Source> {
    let o = read_json(text).filter(|v| v.type_of() == JsonType::Object)?;
    let pyramid = f64::from(GROUND.top())..=f64::from(LEAF_ZOOM);
    let top_zoom = o.get("top_zoom")?.as_f64().filter(|z| z.fract() == 0.0 && pyramid.contains(z))? as Zoom;
    if let Some(format) = o.get("format") {
        read_format(format)?;
    }
    let mut shard = Vec::new();
    for (name, entry) in o.get("shard").filter(|v| v.type_of() == JsonType::Object)?.fields() {
        shard.push(Shard {
            tile: Tile::of_name(name.as_plain()?).filter(|t| t.zoom <= SHARD_ZOOM)?,
            path: paths(entry.get("path"))?,
        });
    }
    Some(Source {
        top_zoom,
        path: paths(o.get("path"))?,
        shard,
    })
}

/// Each kind the source names a format for is in the one this client reads.
fn read_format(v: JsonValue<'_>) -> Option<()> {
    for (kind, n) in v.fields() {
        let layer = BaseLayer::ALL.into_iter().find(|l| kind.eq_str(l.name()))?;
        (n.as_f64()? == f64::from(FORMAT.of(layer))).then_some(())?;
    }
    (v.type_of() == JsonType::Object).then_some(())
}

fn paths(v: Option<JsonValue<'_>>) -> Option<Paths> {
    let mut out = Paths::default();
    let Some(v) = v else { return Some(out) };
    if v.type_of() != JsonType::Object {
        return None;
    }
    for (kind, path) in v.fields() {
        let layer = BaseLayer::ALL.into_iter().find(|l| kind.eq_str(l.name()))?;
        let path = path.as_str()?.as_plain()?;
        let segment = !path.is_empty() && path.len() <= SOURCE_PATH_BYTES && path.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b));
        out[layer as usize] = Some(segment.then(|| path.to_string())?);
    }
    Some(out)
}

#[cfg(test)]
#[path = "source.test.rs"]
mod test;
