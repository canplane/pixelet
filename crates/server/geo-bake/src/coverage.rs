// pxl-geo-bake/coverage.rs

//! What the bakes publish — the shards `infra/geo/elevation/metadata.json`
//! names, a tile at `SHARD_ZOOM` each — which are what the ingests take, and the
//! one door to the DB.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use postgres::{Client, NoTls};
use pxl_geo_spec::table::CATALOG;
use pxl_spec::space::coord::pvf3::ivec::{self, IVec};
use pxl_spec::space::coord::zoom::{count, shamt, units, Zoom, SHARD_ZOOM};
use pxl_spec::space::hierarchy::morton::{self, key_of, key_of_name, node_of};
use pxl_spec::space::hierarchy::tree::{ancestor_of, first_descendant_of};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

const METADATA: &str = include_str!("../../../../infra/geo/elevation/metadata.json");

/// A shard, by its index at `SHARD_ZOOM`.
pub type Shard = [i64; 2];

/// How far past its edges a shard holds, in units: a leaf, as far as any
/// read past a leaf's own reaches (a structure's 3×3, the drawn reach, the
/// elevation's padding), so a shard's leaves bake from the shard alone.
pub const SHARD_APRON: f64 = units(LEAF_ZOOM);

/// A node of the hierarchy at or above the shards: its zoom and index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Node {
    pub zoom: Zoom,
    pub at: [i64; 2],
}

impl Node {
    /// The node's corner nearest the lanes' least, in units.
    pub fn base(self) -> [i64; 2] {
        first_descendant_of(self.at, shamt(self.zoom) as u32)
    }

    pub fn name(self) -> String {
        morton::name::<2>(key_of(self.zoom, self.at))
    }

    /// `[minx, miny, maxx, maxy]` in units, the maximum exclusive.
    pub fn rect(self) -> [f64; 4] {
        let ([x, y], side) = (self.base().map(|v| v as f64), units(self.zoom));
        [x, y, x + side, y + side]
    }

    /// What a shard holds: its rect grown by `SHARD_APRON`.
    pub fn held(self) -> [f64; 4] {
        let [x0, y0, x1, y1] = self.rect();
        [x0 - SHARD_APRON, y0 - SHARD_APRON, x1 + SHARD_APRON, y1 + SHARD_APRON]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coverage {
    shards: BTreeSet<Shard>,
}

static SOURCE: LazyLock<Result<Coverage, String>> = LazyLock::new(Coverage::derive);

impl Coverage {
    pub fn new(shards: impl IntoIterator<Item = Shard>) -> Coverage {
        Coverage { shards: shards.into_iter().collect() }
    }

    pub fn of_source() -> Result<Coverage, String> {
        SOURCE.clone()
    }

    fn derive() -> Result<Coverage, String> {
        let bad = |why: String| format!("infra/geo/elevation/metadata.json: {why}");
        let entries: serde_json::Value = serde_json::from_str(METADATA).map_err(|e| bad(e.to_string()))?;
        let Some([entry]) = entries.as_array().map(Vec::as_slice) else {
            return Err(bad("want an array of exactly one entry".into()));
        };
        let names = entry["shards"].as_array().ok_or_else(|| bad("no shards".into()))?;
        let shards = names
            .iter()
            .map(|n| {
                let name = n.as_str().unwrap_or_default();
                match key_of_name::<2>(name).map(node_of::<2>) {
                    Some((SHARD_ZOOM, at)) => Ok(at),
                    _ => Err(bad(format!("{n} is not a shard's name"))),
                }
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if shards.is_empty() {
            return Err(bad("no shards".into()));
        }
        Ok(Coverage { shards })
    }

    fn shard_of(zoom: Zoom, tile: IVec) -> Shard {
        ancestor_of([ivec::X(tile), ivec::Y(tile)], u32::from(zoom - SHARD_ZOOM))
    }

    pub fn contains(&self, leaf: IVec) -> bool {
        self.shards.contains(&Self::shard_of(LEAF_ZOOM, leaf))
    }

    /// Whether the tile at `zoom` and `tile` covers any leaf held.
    pub fn meets(&self, zoom: Zoom, tile: IVec) -> bool {
        if zoom >= SHARD_ZOOM {
            return self.shards.contains(&Self::shard_of(zoom, tile));
        }
        let at = [ivec::X(tile), ivec::Y(tile)];
        self.shards.iter().any(|&s| ancestor_of(s, u32::from(SHARD_ZOOM - zoom)) == at)
    }

    /// Whether every leaf under the tile at `zoom` and `tile` is held.
    pub fn holds(&self, zoom: Zoom, tile: IVec) -> bool {
        if zoom >= SHARD_ZOOM {
            return self.meets(zoom, tile);
        }
        let at = [ivec::X(tile), ivec::Y(tile)];
        let under = self.shards.iter().filter(|&&s| ancestor_of(s, u32::from(SHARD_ZOOM - zoom)) == at).count();
        under == (count(SHARD_ZOOM, zoom) as usize).pow(2)
    }

    /// Every leaf held.
    pub fn tiles(&self) -> Vec<IVec> {
        let n = i64::from(count(LEAF_ZOOM, SHARD_ZOOM));
        let mut out = Vec::with_capacity(self.shards.len() * (n * n) as usize);
        for &s in &self.shards {
            let [x0, y0] = first_descendant_of(s, u32::from(LEAF_ZOOM - SHARD_ZOOM));
            for y in y0..y0 + n {
                for x in x0..x0 + n {
                    out.push(ivec::gather(x, y, 0));
                }
            }
        }
        out
    }

    /// The shards not held under a top tile that is: what the pyramid's
    /// coarsest tiles show past the coverage.
    pub fn beyond(&self) -> Vec<Tile> {
        let up = u32::from(SHARD_ZOOM - GROUND.top());
        let tops: BTreeSet<[i64; 2]> = self.shards.iter().map(|&s| ancestor_of(s, up)).collect();
        let n = i64::from(count(SHARD_ZOOM, GROUND.top()));
        let mut out = Vec::new();
        for top in tops {
            let [x0, y0] = first_descendant_of(top, up);
            for y in y0..y0 + n {
                for x in x0..x0 + n {
                    if !self.shards.contains(&[x, y]) {
                        out.push(Tile::new(SHARD_ZOOM, ivec::gather(x, y, 0)));
                    }
                }
            }
        }
        out
    }

    pub fn shards(&self) -> impl Iterator<Item = Node> + '_ {
        self.shards.iter().map(|&at| Node { zoom: SHARD_ZOOM, at })
    }

    /// The shard a leaf's bake reads: its own, which holds its apron.
    pub fn around(&self, leaf: IVec) -> BTreeSet<Node> {
        let at = Self::shard_of(LEAF_ZOOM, leaf);
        self.shards.contains(&at).then_some(Node { zoom: SHARD_ZOOM, at }).into_iter().collect()
    }
}

/// The `overture_*` tables a set of statements names, outside string
/// literals and `--` comments, without the catalog; sorted, each once.
pub fn tables_of<'a>(reads: &[&[&'a str]]) -> Vec<&'a str> {
    let mut out: Vec<&str> = Vec::new();
    for sql in reads.iter().flat_map(|r| r.iter()) {
        let b = sql.as_bytes();
        let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
        let (mut i, mut quoted) = (0, false);
        while i < b.len() {
            if b[i] == b'\'' {
                quoted = !quoted;
            } else if !quoted && b[i..].starts_with(b"--") {
                i += b[i..].iter().position(|&c| c == b'\n').unwrap_or(b.len() - i);
                continue;
            } else if !quoted && b[i..].starts_with(b"overture_") && (i == 0 || !word(b[i - 1])) {
                let end = i + b[i..].iter().position(|&c| !word(c)).unwrap_or(b.len() - i);
                out.push(&sql[i..end]);
                i = end;
                continue;
            }
            i += 1;
        }
    }
    out.retain(|t| *t != CATALOG);
    out.sort_unstable();
    out.dedup();
    out
}

/// The geo DB, opened only when every table a bake reads holds the shards it
/// needs, nothing past the coverage's, all of one release. A whole bake needs
/// every shard, since some of its reads are unbounded (the place dictionary).
pub struct Overture {
    url: String,
    pub release: String,
}

const INGEST: &str = "run ./infra/geo/overture/script/ingest.rs";

impl Overture {
    /// For a bake whose statements are `reads` over the shards `need`.
    pub fn reading(reads: &[&[&str]], need: &BTreeSet<Node>) -> Result<(Overture, Client), String> {
        let tables = tables_of(reads);
        let url = std::env::var("DB_GEO_URL").map_err(|_| "env:DB_GEO_URL:missing")?;
        let held: BTreeSet<String> = Coverage::of_source()?.shards().map(Node::name).collect();
        let want: BTreeSet<String> = need.iter().map(|n| n.name()).collect();
        let mut client = Client::connect(&url, NoTls).map_err(|e| e.to_string())?;
        let rows = client
            .query(&format!("SELECT file_name, node, data_date FROM {CATALOG} WHERE file_name = ANY($1)"), &[&tables])
            .map_err(|e| format!("{CATALOG}: {e}: {INGEST}"))?;
        let mut release: Option<(String, String)> = None;
        for &table in &tables {
            let have: BTreeMap<&str, &str> = rows.iter().filter(|r| r.get::<_, &str>(0) == table).map(|r| (r.get(1), r.get(2))).collect();
            let missing = want.iter().filter(|n| !have.contains_key(n.as_str())).count();
            let extra = have.keys().filter(|n| !held.contains(**n)).count();
            if missing + extra > 0 {
                return Err(format!("{table} lacks {missing} of the shards needed and holds {extra} past the coverage's: {INGEST}"));
            }
            for date in have.values() {
                match &release {
                    None => release = Some((table.to_string(), date.to_string())),
                    Some((first, r)) if r != date => return Err(format!("{table} holds release {date} and {first} {r}: one bake reads one release; {INGEST}")),
                    Some(_) => {}
                }
            }
        }
        let (_, release) = release.ok_or("a bake that reads no Overture table")?;
        Ok((Overture { url, release }, client))
    }

    pub fn connect(&self) -> Result<Client, String> {
        Client::connect(&self.url, NoTls).map_err(|e| e.to_string())
    }
}

/// What the geo DB holds for `reads`, the one description of a bake's source:
/// each Overture table it names, the elevation and the coastline. A manifest
/// records it and the dev cache is named by it; an ingest changes it.
pub fn holdings(client: &mut Client, reads: &[&[&str]]) -> Result<serde_json::Value, String> {
    let tables = tables_of(reads);
    let text = |client: &mut Client, sql: &str, params: &[&(dyn postgres::types::ToSql + Sync)]| -> Result<Vec<Vec<Option<String>>>, String> {
        let rows = client.query(sql, params).map_err(|e| format!("{sql}: {e}"))?;
        Ok(rows.iter().map(|r| (0..r.len()).map(|i| r.get::<_, Option<String>>(i)).collect()).collect())
    };
    let overture = text(
        client,
        &format!("SELECT file_name, max(data_date), max(imported_at)::text, sum(total_count)::text FROM {CATALOG} WHERE file_name = ANY($1) GROUP BY file_name ORDER BY file_name"),
        &[&tables],
    )?;
    let elevation = text(client, "SELECT shard, data_date::text, file_name, status, imported_at::text FROM elevation_catalog ORDER BY shard", &[])?;
    // The coastline keeps no catalog; its import makes the table anew.
    let coastline = text(client, "SELECT pg_relation_filenode(to_regclass('osm_land_polygons'))::text", &[])?;
    Ok(serde_json::json!({ "overture": overture, "elevation": elevation, "coastline": coastline }))
}
