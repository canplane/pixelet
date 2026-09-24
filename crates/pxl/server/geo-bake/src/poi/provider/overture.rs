// pxl-geo-bake/poi/provider/overture.rs

use postgres::{Client, IsolationLevel};
use pxl_spec::poi::{xy, Entity};
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::{projection, Pvf3};

/// The source owns its GERS identity and observations. The canonical ID is
/// opaque outside adoption; no spatial or presentation code parses it.
/// The primary key and C ordering make exact identity dedup deterministic.
pub fn visit(client: &mut Client, root: u64, mut accept: impl FnMut(Entity, &str) -> Result<(), String>) -> Result<(), String> {
    let b = xy::bounds(root).map(|v| v * epsg3857::M_PER_U);
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .map_err(|e| e.to_string())?;
    let query = r#"SELECT p.gers_id, p.name, ST_X(p.geom), ST_Y(p.geom), h.z
FROM overture_places p
LEFT JOIN LATERAL (
  SELECT ST_Value(r.rast, 1, p.geom) AS z FROM elevation_raster r
  WHERE r.rast && p.geom AND ST_Intersects(r.rast, p.geom)
    AND ST_Value(r.rast, 1, p.geom) IS NOT NULL
  ORDER BY r.rid LIMIT 1
) h ON true
WHERE p.geom && ST_MakeEnvelope($1,$2,$3,$4,3857)
  AND ST_X(p.geom) >= $1 AND ST_Y(p.geom) >= $2
  AND ST_X(p.geom) < $3 AND ST_Y(p.geom) < $4
ORDER BY p.gers_id COLLATE "C""#;
    let statement = tx.prepare(query).map_err(|e| e.to_string())?;
    let portal = tx.bind(&statement, &[&b[0], &b[1], &b[2], &b[3]]).map_err(|e| e.to_string())?;
    loop {
        let rows = tx.query_portal(&portal, 256).map_err(|e| e.to_string())?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            let id: String = row.get(0);
            let z: Option<f64> = row.get(4);
            let z = z.ok_or_else(|| format!("unresolved elevation for {id}; no complete snapshot published"))?;
            let mut p = Pvf3::default();
            epsg3857::project(&mut p, row.get(2), row.get(3), z);
            let mut position = [0.0; 3];
            projection::unproject(&mut position, p.i, p.f);
            accept(
                Entity {
                    id: id.clone(),
                    position,
                    text: row.get(1),
                },
                &id,
            )?;
        }
    }
    tx.commit().map_err(|e| e.to_string())
}
