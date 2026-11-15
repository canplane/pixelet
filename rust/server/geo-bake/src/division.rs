// pxl-geo-bake/division.rs

//! The divisions layer's leaf, a tile at `DIVISION_LEAF`, read from Overture
//! as it holds them. A cell's center is the point that steps down
//! (pxl-geo-spec `overture::division`), among the children whose points lie
//! within `BAKE_READ_DIST` of the tile. A cell in no area is in none.

use fnv::FnvHashMap;

use pxl_geo_spec::BAKE_READ_DIST;
use pxl_geo_spec::overture::division::{Children, LINEAGE_SQL, steps};

use postgres::Client;
use postgres::types::ToSql;
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::bake::tile::division::{DIVISION_LEAF, Division, DivisionTile, NONE};
use pxl_spec::division::Subtype;
use pxl_spec::space::coord::adapter::epsg3857::{get_tile_bbox, merc_from_u, u_from_merc};
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::{GROUND, TILE_CELLS};
use pxl_spec::space::layer::quadrant::Quadrant;

use crate::pyramid::Base;
use crate::terrain::pipeline::ground::render_polygon;
use crate::wkb::wkb_to_geometries;
use pxl_spec::bake::tile::division;

/// Each area meeting the tile, cut to it, with its division and how deep that is.
const AREAS_SQL: &str = "WITH env AS (SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e)
SELECT a.properties->>'division_id' AS id, jsonb_array_length(d.properties->'hierarchies'->0) AS depth, ST_AsBinary(ST_CollectionExtract(ST_Intersection(a.geom, env.e), 3)) AS geom_wkb
FROM overture_division_areas a JOIN overture_divisions d ON d.gers_id = a.properties->>'division_id', env
WHERE a.geom && env.e AND ST_Intersects(a.geom, env.e)";

/// The divisions with no area whose point lies in the ring, with their parents.
const POINTS_SQL: &str = "SELECT d.gers_id, d.properties->>'parent_division_id' AS parent, ST_X(d.geom) AS x, ST_Y(d.geom) AS y
FROM overture_divisions d
WHERE d.geom && ST_MakeEnvelope($1, $2, $3, $4, 3857)
  AND NOT EXISTS (SELECT 1 FROM overture_division_areas s WHERE s.properties->>'division_id' = d.gers_id)";

/// Every statement the layer runs, for the catalog check.
pub const READS: [&str; 3] = [AREAS_SQL, POINTS_SQL, LINEAGE_SQL];

/// Ids numbered as they are met.
#[derive(Default)]
struct Numbering {
    list: Vec<String>,
    number: FnvHashMap<String, u32>,
}

impl Numbering {
    fn of(&mut self, id: &str) -> u32 {
        if let Some(&k) = self.number.get(id) {
            return k;
        }
        self.list.push(id.to_string());
        let k = (self.list.len() - 1) as u32;
        self.number.insert(id.to_string(), k);
        k
    }
}

/// The divisions leaf of `tile`; none where no cell is in a division.
pub fn leaf(client: &mut Client, tile: [i32; 2]) -> Result<Option<DivisionTile>, String> {
    let db = |e: postgres::Error| e.to_string();
    let [min, max] = get_tile_bbox(DIVISION_LEAF, tile);
    let side = max[0] - min[0];
    let n = TILE_CELLS as usize;
    let step = side / n as f64;
    let cell_u = units(GROUND.cell_zoom(DIVISION_LEAF));
    let local = |p: [f64; 2]| {
        [
            u_from_merc(p[0] - min[0]) / cell_u,
            u_from_merc(p[1] - min[1]) / cell_u,
        ]
    };

    // First, the deepest area over each cell, its id numbered in the order the areas are met.
    let mut ids = Numbering::default();
    let env: [&(dyn ToSql + Sync); 4] = [&min[0], &min[1], &max[0], &max[1]];
    let mut areas: Vec<(i32, String, Vec<u8>)> = client
        .query(AREAS_SQL, &env)
        .map_err(db)?
        .iter()
        .map(|r| {
            (
                r.get::<_, Option<i32>>("depth").unwrap_or(0),
                r.get("id"),
                r.get::<_, &[u8]>("geom_wkb").to_vec(),
            )
        })
        .collect();
    // Shallow first, so the deepest paints last; at one depth the lesser id goes last, so it
    // wins.
    areas.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)));
    let mut cells = vec![NONE; n * n];
    for (_, id, wkb) in &areas {
        let number = ids.of(id);
        for g in wkb_to_geometries(wkb)
            .into_iter()
            .filter(|g| g.points.len() >= 3)
        {
            let points: Vec<[f64; 2]> = g.points.iter().map(|p| local([p[0], p[1]])).collect();
            let holes: Vec<Vec<[f64; 2]>> = g
                .holes
                .iter()
                .map(|h| h.iter().map(|p| local([p[0], p[1]])).collect())
                .collect();
            render_polygon(&points, number, &mut cells, &holes, None);
        }
    }

    // Then each cell under an area steps down, as far as it can, to the child whose point lies
    // nearest it, among the children whose points lie within the reach.
    let margin = merc_from_u(BAKE_READ_DIST);
    let ring: [&(dyn ToSql + Sync); 4] = [
        &(min[0] - margin),
        &(min[1] - margin),
        &(max[0] + margin),
        &(max[1] + margin),
    ];
    let mut children = Children::default();
    for r in client.query(POINTS_SQL, &ring).map_err(db)? {
        if let Some(parent) = r.get::<_, Option<String>>("parent") {
            children
                .entry(parent)
                .or_default()
                .push(([r.get("x"), r.get("y")], r.get("gers_id")));
        }
    }
    for idx in 0..(n * n) as u32 {
        let mut number = cells[idx as usize];
        if number == NONE {
            continue;
        }
        let [tx, ty] = GROUND.in_tile(idx);
        let center = [
            min[0] + (f64::from(tx) + 0.5) * step,
            min[1] + (f64::from(ty) + 0.5) * step,
        ];
        for id in steps(&ids.list[number as usize].clone(), center, &children) {
            number = ids.of(id);
        }
        cells[idx as usize] = number;
    }
    if cells.iter().all(|&c| c == NONE) {
        return Ok(None);
    }

    // Last, each division a cell names is read, with its parents.
    let rows = client.query(LINEAGE_SQL, &[&ids.list]).map_err(db)?;
    let mut recs: FnvHashMap<&str, (Division, Option<&str>)> = FnvHashMap::default();
    for r in &rows {
        let Some(kind) = r
            .get::<_, Option<&str>>("subtype")
            .and_then(Subtype::of_word)
        else {
            continue;
        };
        let id: &str = r.get("gers_id");
        let division = Division {
            id,
            kind,
            name: r.get::<_, Option<&str>>("name").unwrap_or_default(),
            parent: NONE,
            prominence: r
                .get::<_, Option<i32>>("prominence")
                .unwrap_or(0)
                .clamp(0, 100) as u8,
            area: r.get("area"),
        };
        recs.insert(id, (division, r.get("parent")));
    }
    // The divisions in id order, so a tile is the same whatever order its rows came in.
    let mut order: Vec<&str> = recs.keys().copied().collect();
    order.sort();
    let idx_of: FnvHashMap<&str, u32> = order
        .iter()
        .enumerate()
        .map(|(k, id)| (*id, k as u32))
        .collect();
    let divisions: Vec<Division> = order
        .iter()
        .map(|id| {
            let (d, parent) = &recs[*id];
            Division {
                parent: parent.and_then(|p| idx_of.get(p)).copied().unwrap_or(NONE),
                ..*d
            }
        })
        .collect();
    Ok(Some(DivisionTile::of(
        cells
            .into_iter()
            .map(|c| {
                if c == NONE {
                    NONE
                } else {
                    ids.list
                        .get(c as usize)
                        .and_then(|id| idx_of.get(id.as_str()))
                        .copied()
                        .unwrap_or(NONE)
                }
            })
            .collect::<Vec<_>>()
            .into(),
        divisions,
        Occupancy::ZERO,
    )))
}

impl Base for DivisionTile {
    const PRUNED: bool = true;

    fn encode(&self, at: Quadrant) -> Result<Vec<u8>, String> {
        division::encode(self).map_err(|e| format!("{}: {e:?}", at.key()))
    }

    fn decode(raw: &[u8]) -> Result<Self, String> {
        division::decode(raw).map_err(|e| format!("{e:?}"))
    }

    fn fold_tile<'a>(p: Quadrant, child_of: impl Fn(Quadrant) -> Option<&'a Self>) -> Option<Self> {
        let [a, b, c, d] = p.children().map(child_of);
        Some(division::fold([a?, b?, c?, d?], p.zoom))
    }
}
