// pxl-geo-bake/geocode/provider/overture.rs

//! Overture's divisions at each tile's center, by the rules `pxl-geo`
//! answers a search with (`pxl-geo-spec overture/division.rs`).

use std::collections::HashMap;

use postgres::{Client, Row, Statement};
use pxl_geo_spec::overture::division::{self, FINEST_SQL, LINEAGE_SQL};
use pxl_spec::bake::division_tile::{leaf_slot, Division, DivisionGroup, APPROX, GROUP_TILES, LOCAL, NONE};
use pxl_spec::space::coord::adapter::epsg3857;
use pxl_spec::space::coord::pvf3::ivec::{self, IVec};
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use crate::geocode::{DivisionDict, Entry};

/// The divisions an area is of or another is the child of: every one a
/// tile's name can walk up through. A stable order, so a resumed bake
/// numbers them the same way.
const DICTIONARY_SQL: &str = concat!(
    "SELECT d.gers_id, d.subtype, d.name, ",
    pxl_geo_spec::division_names!("d"),
    " AS names_common, d.properties->>'parent_division_id' AS parent
FROM overture_divisions d
WHERE d.gers_id IN (SELECT properties->>'division_id' FROM overture_division_areas UNION SELECT properties->>'parent_division_id' FROM overture_divisions)
ORDER BY d.gers_id"
);

/// Every statement the provider runs, for the catalog check.
pub const READS: [&str; 3] = [FINEST_SQL, LINEAGE_SQL, DICTIONARY_SQL];

pub struct GeocodeProvider {
    finest: Statement,
    lineage: Statement,
}

fn division_of(row: &Row) -> division::Division {
    division::Division::new(row.get("gers_id"), row.get::<_, &str>("subtype"), row.get("name"), row.get("names_common"), row.get("parent"))
}

/// As the tile holds it, its parent filled in by whoever numbers it; none
/// for a subtype newer than this tree.
fn entry(d: division::Division) -> Option<Entry> {
    let subtype = d.subtype?;
    Some((
        d.gers_id,
        d.parent,
        Division {
            subtype,
            name: d.name,
            names: d.names.into_iter().collect(),
            parent: NONE,
        },
    ))
}

impl GeocodeProvider {
    pub fn prepare(client: &mut Client) -> Result<GeocodeProvider, postgres::Error> {
        Ok(GeocodeProvider {
            finest: client.prepare(FINEST_SQL)?,
            lineage: client.prepare(LINEAGE_SQL)?,
        })
    }

    pub fn dictionary(&self, client: &mut Client) -> Result<Vec<Entry>, postgres::Error> {
        Ok(client.query(DICTIONARY_SQL, &[])?.iter().map(division_of).filter_map(entry).collect())
    }

    /// Each leaf's division, by its center: one the dictionary has by its
    /// number, another the group's own, under its parent, which the
    /// dictionary has, since it is a parent.
    pub fn group(&self, client: &mut Client, dict: &DivisionDict, leaves: &[IVec]) -> Result<Option<DivisionGroup>, String> {
        let db = |e: postgres::Error| e.to_string();
        let (xs, ys): (Vec<f64>, Vec<f64>) = leaves
            .iter()
            .map(|&leaf| {
                let bb = epsg3857::get_tile_bbox(LEAF_ZOOM, leaf);
                ((bb[0][0] + bb[1][0]) / 2.0, (bb[0][1] + bb[1][1]) / 2.0)
            })
            .unzip();
        let reach: Vec<f64> = ys.iter().map(|&y| division::reach_at(y)).collect();
        let rows = client.query(&self.finest, &[&xs, &ys, &reach]).map_err(db)?;
        let finest = division::by_point(leaves.len(), rows.iter().map(|r| (r.get("idx"), r.get("division_id"), r.get("approx"))));
        let own: Vec<String> = finest.iter().flatten().map(|(id, _)| id.clone()).filter(|id| dict.number(id).is_none()).collect();
        let records: HashMap<String, Entry> =
            if own.is_empty() { HashMap::new() } else { client.query(&self.lineage, &[&own]).map_err(db)?.iter().map(division_of).filter_map(entry).map(|e| (e.0.clone(), e)).collect() };

        let mut group = DivisionGroup {
            tiles: vec![NONE; GROUP_TILES],
            locals: Vec::new(),
        };
        let mut local_at: HashMap<String, u32> = HashMap::new();
        for (&leaf, found) in leaves.iter().zip(finest) {
            let Some((id, approx)) = found else { continue };
            let n = match dict.number(&id) {
                Some(n) => n,
                None => {
                    let Some((_, parent, d)) = records.get(&id) else { continue };
                    *local_at.entry(id).or_insert_with(|| {
                        group.locals.push(Division {
                            parent: parent.as_deref().and_then(|p| dict.number(p)).unwrap_or(NONE),
                            ..d.clone()
                        });
                        (group.locals.len() - 1) as u32
                    }) | LOCAL
                }
            };
            group.tiles[leaf_slot(ivec::X(leaf), ivec::Y(leaf))] = n | if approx { APPROX } else { 0 };
        }
        Ok(group.tiles.iter().any(|&n| n != NONE).then_some(group))
    }
}
