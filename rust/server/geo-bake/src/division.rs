// pxl-geo-bake/division.rs

//! The divisions layer's leaf, a tile at `DIVISION_LEAF`, read from Overture
//! as it holds them. A cell is in the deepest division whose area holds its
//! center; under that, in the child with no area whose point is nearest,
//! and so on down, the points looked for within the bakes' reach of the tile
//! (`coverage.rs` `BAKE_REACH`) and no further. A cell in no area is in none.
//! Equal distances go to the lesser id, and a division already on the way
//! down is not taken again.

use fnv::FnvHashMap;

use postgres::types::ToSql;
use postgres::Client;
use pxl_spec::bake::tile::division::{Division, DivisionTile, DIVISION_LEAF, NONE};
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::division::Subtype;
use pxl_spec::space::coord::adapter::epsg3857::{get_tile_bbox, mer_from_u, u_from_mer};
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::{GROUND, TILE_CELLS};
use pxl_spec::space::layer::tile::Tile;

use crate::coverage::BAKE_REACH;
use crate::terrain::pipeline::ground::render_polygon;
use crate::wkb::wkb_to_geometries;

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

/// The divisions `$1` names and every parent of each.
const RECORDS_SQL: &str = concat!(
    "WITH RECURSIVE up AS (
  SELECT d.gers_id, d.properties FROM overture_divisions d WHERE d.gers_id = ANY($1)
  UNION SELECT p.gers_id, p.properties FROM overture_divisions p JOIN up ON p.gers_id = up.properties->>'parent_division_id'
)
SELECT up.gers_id, up.properties->>'subtype' AS subtype, up.properties->'names'->>'primary' AS name, ",
    pxl_geo_spec::division_names!("up"),
    " AS names_common, up.properties->>'parent_division_id' AS parent,
  (up.properties->'cartography'->>'prominence')::int AS prominence,
  EXISTS (SELECT 1 FROM overture_division_areas s WHERE s.properties->>'division_id' = up.gers_id) AS area
FROM up"
);

/// Every statement the layer runs, for the catalog check.
pub const READS: [&str; 3] = [AREAS_SQL, POINTS_SQL, RECORDS_SQL];

/// Ids numbered as they are met.
#[derive(Default)]
struct Numbered {
    list: Vec<String>,
    number: FnvHashMap<String, u32>,
}

impl Numbered {
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

/// None where no cell is in a division.
pub fn leaf(client: &mut Client, tile: [i32; 2]) -> Result<Option<DivisionTile>, String> {
    let db = |e: postgres::Error| e.to_string();
    let [min, max] = get_tile_bbox(DIVISION_LEAF, tile);
    let side = max[0] - min[0];
    let n = TILE_CELLS as usize;
    let step = side / n as f64;
    let cell_u = units(GROUND.cell_zoom(DIVISION_LEAF));
    let local = |p: [f64; 2]| [u_from_mer(p[0] - min[0]) / cell_u, u_from_mer(p[1] - min[1]) / cell_u];

    // ---- the deepest area over each cell; ids numbered as they are met ----
    let mut ids = Numbered::default();
    let env: [&(dyn ToSql + Sync); 4] = [&min[0], &min[1], &max[0], &max[1]];
    let mut areas: Vec<(i32, String, Vec<u8>)> = client
        .query(AREAS_SQL, &env)
        .map_err(db)?
        .iter()
        .map(|r| (r.get::<_, Option<i32>>("depth").unwrap_or(0), r.get("id"), r.get::<_, &[u8]>("geom_wkb").to_vec()))
        .collect();
    // shallow first, so the deepest paints last; at one depth the lesser id last, so it wins
    areas.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)));
    let mut cells = vec![NONE; n * n];
    for (_, id, wkb) in &areas {
        let at = ids.of(id);
        for g in wkb_to_geometries(wkb).into_iter().filter(|g| g.points.len() >= 3) {
            let points: Vec<[f64; 2]> = g.points.iter().map(|p| local([p[0], p[1]])).collect();
            let holes: Vec<Vec<[f64; 2]>> = g.holes.iter().map(|h| h.iter().map(|p| local([p[0], p[1]])).collect()).collect();
            render_polygon(&points, at, &mut cells, &holes, None);
        }
    }

    // ---- under the areas, the nearest point of a child, within the reach ----
    let reach = mer_from_u(BAKE_REACH);
    let ring: [&(dyn ToSql + Sync); 4] = [&(min[0] - reach), &(min[1] - reach), &(max[0] + reach), &(max[1] + reach)];
    let mut children: FnvHashMap<String, Vec<([f64; 2], String)>> = FnvHashMap::default();
    for r in client.query(POINTS_SQL, &ring).map_err(db)? {
        if let Some(parent) = r.get::<_, Option<String>>("parent") {
            children.entry(parent).or_default().push(([r.get("x"), r.get("y")], r.get("gers_id")));
        }
    }
    for kids in children.values_mut() {
        kids.sort_by(|a, b| a.1.cmp(&b.1));
    }
    for index in 0..(n * n) as u32 {
        let mut at = cells[index as usize];
        if at == NONE {
            continue;
        }
        let [tx, ty] = GROUND.in_tile(index);
        let center = [min[0] + (f64::from(tx) + 0.5) * step, min[1] + (f64::from(ty) + 0.5) * step];
        let mut taken = vec![at];
        while let Some(kids) = children.get(&ids.list[at as usize]) {
            let nearest = kids
                .iter()
                .filter(|(_, id)| ids.number.get(id).is_none_or(|k| !taken.contains(k)))
                .map(|(p, id)| {
                    let [dx, dy] = [p[0] - center[0], p[1] - center[1]];
                    (dx * dx + dy * dy, id)
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            let Some((_, id)) = nearest else { break };
            at = ids.of(id);
            taken.push(at);
        }
        cells[index as usize] = at;
    }
    if cells.iter().all(|&c| c == NONE) {
        return Ok(None);
    }

    // ---- each division a cell names, with its parents ----
    let rows = client.query(RECORDS_SQL, &[&ids.list]).map_err(db)?;
    let mut records: FnvHashMap<String, (Division, Option<String>)> = FnvHashMap::default();
    for r in &rows {
        let Some(subtype) = r.get::<_, Option<&str>>("subtype").and_then(Subtype::named) else { continue };
        let names = match r.get::<_, Option<serde_json::Value>>("names_common") {
            Some(serde_json::Value::Object(map)) => map.into_iter().filter_map(|(tag, name)| name.as_str().map(|n| (tag, n.to_string()))).collect(),
            _ => Vec::new(),
        };
        let id: String = r.get("gers_id");
        let division = Division {
            id: id.clone(),
            subtype,
            name: r.get::<_, Option<String>>("name").unwrap_or_default(),
            names,
            parent: NONE,
            prominence: r.get::<_, Option<i32>>("prominence").unwrap_or(0).clamp(0, 100) as u8,
            area: r.get("area"),
        };
        records.insert(id, (division, r.get("parent")));
    }
    // the divisions in id order, so a tile is the same whatever order its rows came in
    let mut order: Vec<&String> = records.keys().collect();
    order.sort();
    let at: FnvHashMap<&String, u32> = order.iter().enumerate().map(|(k, id)| (*id, k as u32)).collect();
    let divisions: Vec<Division> = order
        .iter()
        .map(|id| {
            let (d, parent) = &records[*id];
            Division {
                parent: parent.as_ref().and_then(|p| at.get(p)).copied().unwrap_or(NONE),
                ..d.clone()
            }
        })
        .collect();
    Ok(Some(DivisionTile {
        cells: cells
            .into_iter()
            .map(|c| {
                if c == NONE {
                    NONE
                } else {
                    ids.list.get(c as usize).and_then(|id| at.get(id)).copied().unwrap_or(NONE)
                }
            })
            .collect(),
        divisions,
        occupancy: Occupancy::ZERO,
    }))
}

impl crate::pyramid::Base for DivisionTile {
    const LAYER: pxl_spec::bake::published::BaseLayer = pxl_spec::bake::published::BaseLayer::division;

    const LEAF: pxl_spec::space::coord::zoom::Zoom = DIVISION_LEAF;

    const LATTICE: pxl_spec::space::layer::lattice::Lattice = pxl_spec::space::layer::lattice::GROUND;

    fn encode(&self, _: Tile) -> Result<Vec<u8>, String> {
        pxl_spec::bake::tile::division::encode(self).map_err(|e| format!("{e:?}"))
    }

    fn decode(raw: &[u8]) -> Result<Self, String> {
        pxl_spec::bake::tile::division::decode(raw).map_err(|e| format!("{e:?}"))
    }

    fn fold_tile<'a>(p: Tile, held: impl Fn(Tile) -> Option<&'a Self>) -> Option<Self> {
        let [a, b, c, d] = p.children().map(held);
        Some(pxl_spec::bake::tile::division::fold([a?, b?, c?, d?], p.zoom))
    }

    fn occupancy(&self) -> Occupancy {
        self.occupancy
    }
}
