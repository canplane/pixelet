// pxl-geo-bake/source.rs

//! The tile source's declaration, the one writer of what pxl-kernel
//! sys/io/source.rs reads: the coarsest zoom supplied whole, each kind's format
//! and version, and the shards every kind holds, four siblings written as their
//! parent, with each override's version. The published one is made from a
//! record of what the bucket serves (bin/declare_source.rs, which
//! script/publish.ts gathers out of the bucket and uploads), a dev tile
//! server's from what it serves (bin/serve_tiles.rs).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use pxl_spec::bake::published::BaseLayer;
use pxl_spec::bake::source::{is_version, key, sharded};
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::coord::zoom::SHARD_ZOOM;
use pxl_spec::space::layer::tile::{Tile, TOP_ZOOM};
use serde_json::{json, Map, Value};

use crate::pyramid::MANIFEST;

/// What a source says of one kind: the format it is in, where it is (the
/// version a tile's name is under, which no client reads) and the shards
/// it holds; the Earth's picture is no shard's.
#[derive(Clone)]
pub struct Kind {
    pub format: u32,
    pub version: String,
    pub shard: Option<BTreeSet<Tile>>,
}

pub type Kinds = [Kind; BaseLayer::ALL.len()];

/// Each kind's, in `BaseLayer`'s order.
pub fn try_kinds(kind: impl FnMut(BaseLayer) -> Result<Kind, String>) -> Result<Kinds, String> {
    let all: Vec<Kind> = BaseLayer::ALL.into_iter().map(kind).collect::<Result<_, _>>()?;
    Ok(all.try_into().unwrap_or_else(|_| unreachable!("one a kind")))
}

/// A kind's versions for a shard and the tiles under it, over the kind's own.
pub type Overrides = BTreeMap<Tile, Vec<(BaseLayer, String)>>;

/// Refused for anything a client would refuse (pxl-spec bake/source.rs), or
/// would serve wrongly: a version that is no URL segment, a sharded kind holding no shards
/// or another holding some, an override on anything but a shard served.
pub fn declaration(top_zoom: Zoom, kinds: &Kinds, overrides: &Overrides) -> Result<Value, String> {
    for l in BaseLayer::ALL {
        let k = &kinds[l as usize];
        if k.shard.is_some() != sharded(l) {
            return Err(format!("{}: {}", l.name(), if sharded(l) { "no shards it holds" } else { "is held by no shards" }));
        }
        if !is_version(&k.version) {
            return Err(format!("{}: {:?} is no version a client reads", l.name(), k.version));
        }
    }
    let held: Vec<&BTreeSet<Tile>> = kinds.iter().filter_map(|k| k.shard.as_ref()).collect();
    let every: BTreeSet<Tile> = held
        .first()
        .map(|first| first.iter().filter(|s| held.iter().all(|h| h.contains(s))).copied().collect())
        .unwrap_or_default();
    for (t, versions) in overrides {
        // A key is served whatever it carries: one on no shard served would serve what is not.
        if !every.contains(t) || versions.iter().any(|(l, p)| !sharded(*l) || !is_version(p)) {
            return Err(format!("override {}: no shard every kind holds, or no version a client reads", t.name()));
        }
    }
    let named = |f: &dyn Fn(&Kind) -> Value| -> Map<String, Value> { BaseLayer::ALL.into_iter().map(|l| (l.name().to_string(), f(&kinds[l as usize]))).collect() };
    let mut shard: Map<String, Value> = folded(every).into_iter().map(|t| (t.name(), json!({}))).collect();
    for (t, versions) in overrides {
        let version: Map<String, Value> = versions.iter().map(|(l, p)| (l.name().to_string(), json!(p))).collect();
        shard.insert(t.name(), json!({ "version": version }));
    }
    Ok(json!({ "top_zoom": top_zoom, "shard": shard, "format": named(&|k| json!(k.format)), "version": named(&|k| json!(k.version)) }))
}

/// Four siblings all held, written as their parent, up to the pyramid's top.
fn folded(mut keys: BTreeSet<Tile>) -> BTreeSet<Tile> {
    let finest = keys.iter().map(|t| t.zoom).max().unwrap_or(TOP_ZOOM);
    for zoom in (TOP_ZOOM + 1..=finest).rev() {
        let parents: BTreeSet<Tile> = keys.iter().filter(|t| t.zoom == zoom).map(|t| t.parent()).collect();
        for p in parents {
            if p.children().iter().all(|c| keys.contains(c)) {
                p.children().iter().for_each(|c| {
                    keys.remove(c);
                });
                keys.insert(p);
            }
        }
    }
    keys
}

/// Shards by name: a shard's each, no node over them.
pub fn tiles<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<BTreeSet<Tile>, String> {
    names.into_iter().map(|n| key(n).filter(|t| t.zoom == SHARD_ZOOM).ok_or(format!("{n}: no shard's name"))).collect()
}

/// A set's manifest; none where the set has none yet.
pub fn manifest(set: &Path) -> Result<Option<Value>, String> {
    let path = set.join(MANIFEST);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map(Some).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// The shards a set's manifest names it holds; none where it names none.
pub fn held(manifest: &Value) -> Result<Option<BTreeSet<Tile>>, String> {
    manifest["shard"].as_array().map(|names| tiles(names.iter().filter_map(Value::as_str))).transpose()
}

/// What a set's manifest says of the set, served at `version`: its format and
/// the shards it holds.
pub fn of_set(set: &Path, version: &str) -> Result<Kind, String> {
    let manifest = manifest(set)?.ok_or(format!("{}: no {MANIFEST}", set.display()))?;
    Ok(Kind {
        format: manifest["format"].as_u64().ok_or(format!("{}: its {MANIFEST} names no format", set.display()))? as u32,
        version: version.to_string(),
        shard: held(&manifest)?,
    })
}

/// The published declaration from a record of what the bucket serves:
/// `kind`, each kind's version and what its manifest says of it, its
/// `format` and `shard`; `override`, a kind's version for a shard.
pub fn recorded(record: &Value) -> Result<Value, String> {
    let kind = |l: BaseLayer| -> Result<Kind, String> {
        let k = &record["kind"][l.name()];
        let at = |field: &str| format!("kind.{}.{field}", l.name());
        Ok(Kind {
            format: k["format"].as_u64().ok_or_else(|| at("format"))? as u32,
            version: k["version"].as_str().ok_or_else(|| at("version"))?.to_string(),
            shard: held(k)?,
        })
    };
    let kinds = try_kinds(kind)?;
    let mut overrides = Overrides::new();
    for (name, versions) in record["override"].as_object().into_iter().flatten() {
        let tile = key(name).ok_or(format!("override.{name}: no shard's name"))?;
        let mut own = Vec::new();
        for (kind, version) in versions.as_object().into_iter().flatten() {
            let layer = BaseLayer::ALL.into_iter().find(|l| l.name() == kind).ok_or(format!("override.{name}.{kind}: no kind"))?;
            own.push((layer, version.as_str().ok_or(format!("override.{name}.{kind}: no version"))?.to_string()));
        }
        overrides.insert(tile, own);
    }
    declaration(TOP_ZOOM, &kinds, &overrides)
}

#[cfg(test)]
#[path = "source.test.rs"]
mod test;
