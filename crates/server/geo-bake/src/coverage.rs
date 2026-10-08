// pxl-geo-bake/coverage.rs

//! What the bakes publish — the shards `infra/geo/elevation/metadata.json`
//! names, a tile at `SHARD_ZOOM` each — which are what the ingests take, and the
//! one door to the DB.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use postgres::{Client, NoTls};
use pxl_geo_spec::table::CATALOG;
use pxl_spec::space::coord::pvf3::ivec::{self, IVec};
use pxl_spec::space::coord::zoom::{count, shamt, units, Zoom, SHARD_ZOOM};
use pxl_spec::space::hierarchy::morton::{self, key_of, key_of_name, node_of};
use pxl_spec::space::hierarchy::tree::{ancestor_of, first_descendant_of};
use pxl_spec::space::layer::tile::LEAF_ZOOM;

const METADATA: &str = include_str!("../../../../infra/geo/elevation/metadata.json");

/// A shard, by its index at `SHARD_ZOOM`.
pub type Shard = [i64; 2];

/// What an edge may be off by, in units: a ground cell, the finest thing a
/// bake tells apart, and more than the meter a shard's edges and a feature's
/// extent are each rounded to.
pub const EPS: f64 = units(GROUND.cell());

/// How far past a leaf any bake reads, in units: a z13 tile, four terrain
/// leaves. A terrain leaf's reads stay within its 2-ring (AGENTS.md); a
/// divisions leaf looks this far for the point of a division with no area,
/// past the gap between such points nearly everywhere (99% of Korea's
/// neighbourhoods, most of its villages, measured 2026-10-08).
pub const BAKE_REACH: f64 = units(LEAF_ZOOM - 2);

const _: () = assert!(2.0 * units(LEAF_ZOOM) <= BAKE_REACH, "a terrain leaf's 2-ring reads past the bakes' reach");

/// How far past its edges a shard holds, in units: the bakes' reach and
/// `EPS`, so every leaf bakes from its shard alone. A read past `BAKE_REACH` is a
/// violation, never a reason to grow this.
pub const SHARD_APRON: f64 = BAKE_REACH + EPS;

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

    /// Its tiles at `zoom`, at or under it.
    pub fn tiles(self, zoom: Zoom) -> Vec<IVec> {
        let shift = u32::from(zoom - self.zoom);
        let (n, [x0, y0]) = (1i64 << shift, first_descendant_of(self.at, shift));
        (y0..y0 + n).flat_map(|y| (x0..x0 + n).map(move |x| ivec::gather(x, y, 0))).collect()
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
        self.shards().flat_map(|s| s.tiles(LEAF_ZOOM)).collect()
    }

    pub fn shards(&self) -> impl Iterator<Item = Node> + '_ {
        self.shards.iter().map(|&at| Node { zoom: SHARD_ZOOM, at })
    }

    /// The shard a tile's bake reads: its own, which holds its apron.
    pub fn around(&self, zoom: Zoom, tile: IVec) -> BTreeSet<Node> {
        let at = Self::shard_of(zoom, tile);
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
/// needs and nothing past the coverage's.
pub struct Overture {
    url: String,
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
            .query(&format!("SELECT file_name, node FROM {CATALOG} WHERE file_name = ANY($1)"), &[&tables])
            .map_err(|e| format!("{CATALOG}: {e}: {INGEST}"))?;
        for &table in &tables {
            let have: BTreeSet<&str> = rows.iter().filter(|r| r.get::<_, &str>(0) == table).map(|r| r.get(1)).collect();
            let missing = want.iter().filter(|n| !have.contains(n.as_str())).count();
            let extra = have.iter().filter(|n| !held.contains(**n)).count();
            if missing + extra > 0 {
                return Err(format!("{table} lacks {missing} of the shards needed and holds {extra} past the coverage's: {INGEST}"));
            }
        }
        Ok((Overture { url }, client))
    }

    pub fn connect(&self) -> Result<Client, String> {
        Client::connect(&self.url, NoTls).map_err(|e| e.to_string())
    }
}

/// What the geo DB holds for `reads`, which a set's manifest says it was
/// made from: each Overture table it names by its release and counts, and
/// the elevation.
pub fn holdings(client: &mut Client, reads: &[&[&str]]) -> Result<serde_json::Value, String> {
    let tables = tables_of(reads);
    let text = |client: &mut Client, sql: &str, params: &[&(dyn postgres::types::ToSql + Sync)]| -> Result<Vec<Vec<Option<String>>>, String> {
        let rows = client.query(sql, params).map_err(|e| format!("{sql}: {e}"))?;
        Ok(rows.iter().map(|r| (0..r.len()).map(|i| r.get::<_, Option<String>>(i)).collect()).collect())
    };
    let overture = text(
        client,
        &format!("SELECT file_name, max(data_date), sum(total_count)::text FROM {CATALOG} WHERE file_name = ANY($1) GROUP BY file_name ORDER BY file_name"),
        &[&tables],
    )?;
    let elevation = text(client, "SELECT shard, data_date::text, file_name, status FROM elevation_catalog ORDER BY shard", &[])?;
    Ok(serde_json::json!({ "overture": overture, "elevation": elevation }))
}

/// `holdings`, for a set's manifest.
pub fn source_of(reads: &[&[&str]]) -> Result<serde_json::Value, String> {
    let (_, mut client) = Overture::reading(reads, &BTreeSet::new())?;
    holdings(&mut client, reads)
}
