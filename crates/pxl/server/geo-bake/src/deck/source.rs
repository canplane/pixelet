// pxl-geo-bake/deck/source.rs

//! What the solve reads from the geo DB and the table it writes.
//!
//! The geometry questions — what crosses a deck, where along it, over which
//! water — are PostGIS's, asked of the pieces staged in a temporary table so
//! that every one is a join on an index; what the answers mean is the
//! solve's (deck.rs).

use postgres::Client;

use crate::deck::piece::{BridgeRow, Piece};

/// Rows a batched insert or sample sends at once.
const BATCH: usize = 20_000;

/// Every segment a bridge flag is on.
pub fn bridge_rows(client: &mut Client) -> Result<Vec<BridgeRow>, postgres::Error> {
    let rows = client.query(
        "SELECT gers_id, subtype, class, COALESCE(road_flags, rail_flags)::text AS flags, level_rules::text AS levels, ST_AsBinary(geom) AS geom_wkb
         FROM overture_transportation
         WHERE COALESCE(road_flags, rail_flags)::text LIKE '%is_bridge%'",
        &[],
    )?;
    Ok(rows
        .iter()
        .map(|r| BridgeRow {
            gers_id: r.get("gers_id"),
            subtype: r.get("subtype"),
            class: r.get("class"),
            flags: r.get("flags"),
            levels: r.get("levels"),
            geom_wkb: r.get::<_, &[u8]>("geom_wkb").into(),
        })
        .collect())
}

fn wkt(points: impl Iterator<Item = String>, z: bool) -> String {
    format!("LINESTRING{} ({})", if z { " Z" } else { "" }, points.collect::<Vec<_>>().join(","))
}

/// The pieces as `deck_piece`, a temporary table indexed for the joins below.
pub fn stage(client: &mut Client, pieces: &[Piece]) -> Result<(), postgres::Error> {
    client.batch_execute(
        "DROP TABLE IF EXISTS deck_piece;
         CREATE TEMP TABLE deck_piece (id INT PRIMARY KEY, gers_id TEXT NOT NULL, level INT NOT NULL, geom GEOMETRY(LineString, 3857) NOT NULL);",
    )?;
    for (b, batch) in pieces.chunks(BATCH).enumerate() {
        let ids: Vec<i32> = (0..batch.len()).map(|i| (b * BATCH + i) as i32).collect();
        let gers: Vec<&str> = batch.iter().map(|p| p.gers_id.as_str()).collect();
        let levels: Vec<i32> = batch.iter().map(|p| p.level as i32).collect();
        let lines: Vec<String> = batch.iter().map(|p| wkt(p.points.iter().map(|q| format!("{} {}", q[0], q[1])), false)).collect();
        client.execute(
            "INSERT INTO deck_piece SELECT id, g, l, ST_GeomFromText(w, 3857) FROM unnest($1::int[], $2::text[], $3::int[], $4::text[]) AS t(id, g, l, w)",
            &[&ids, &gers, &levels, &lines],
        )?;
    }
    client.batch_execute("CREATE INDEX ON deck_piece USING GIST (geom); ANALYZE deck_piece;")
}

/// A way that crosses a piece: where along the piece, in metres, what it is,
/// and where along itself, as a fraction, so its own flags can say whether
/// it is under the deck there or a tunnel or a deck of its own.
pub struct WayCrossing {
    pub piece: usize,
    pub at: f64,
    pub subtype: String,
    pub class: Option<String>,
    pub flags: Option<String>,
    pub own_at: f64,
}

pub fn ways_crossing(client: &mut Client) -> Result<Vec<WayCrossing>, postgres::Error> {
    let rows = client.query(
        "SELECT p.id, ST_LineLocatePoint(p.geom, d.geom) * ST_Length(p.geom) AS at,
                t.subtype, t.class, COALESCE(t.road_flags, t.rail_flags)::text AS flags,
                ST_LineLocatePoint(t.geom, d.geom) AS own_at
         FROM deck_piece p
         JOIN overture_transportation t ON t.geom && p.geom AND t.gers_id <> p.gers_id AND ST_Intersects(t.geom, p.geom)
         CROSS JOIN LATERAL ST_Dump(ST_Intersection(t.geom, p.geom)) d
         WHERE GeometryType(d.geom) = 'POINT'",
        &[],
    )?;
    Ok(rows
        .iter()
        .map(|r| WayCrossing {
            piece: r.get::<_, i32>("id") as usize,
            at: r.get("at"),
            subtype: r.get("subtype"),
            class: r.get("class"),
            flags: r.get("flags"),
            own_at: r.get("own_at"),
        })
        .collect())
}

/// Water under a piece: from and to, in metres along it — the same where a
/// line of water crosses it — and the water's subtype and class.
pub struct WaterUnder {
    pub piece: usize,
    pub from: f64,
    pub to: f64,
    pub subtype: String,
    pub class: Option<String>,
}

pub fn water_under(client: &mut Client) -> Result<Vec<WaterUnder>, postgres::Error> {
    let rows = client.query(
        "SELECT p.id,
                ST_LineLocatePoint(p.geom, ST_StartPoint(d.geom)) * ST_Length(p.geom) AS a,
                ST_LineLocatePoint(p.geom, ST_EndPoint(d.geom)) * ST_Length(p.geom) AS b,
                w.subtype, w.class
         FROM deck_piece p
         JOIN overture_base_water w ON w.geom && p.geom AND w.subtype <> 'ocean' AND ST_Intersects(w.geom, p.geom)
         CROSS JOIN LATERAL ST_Dump(ST_Intersection(p.geom, w.geom)) d
         WHERE GeometryType(d.geom) IN ('POINT', 'LINESTRING')",
        &[],
    )?;
    Ok(rows
        .iter()
        .map(|r| {
            let (a, b): (f64, f64) = (r.get("a"), r.get("b"));
            WaterUnder {
                piece: r.get::<_, i32>("id") as usize,
                from: a.min(b),
                to: a.max(b),
                subtype: r.get("subtype"),
                class: r.get("class"),
            }
        })
        .collect())
}

/// Where a piece crosses one of a lower level: the upper's piece and how far
/// along it, the lower's and how far along it, in metres.
pub struct DeckCrossing {
    pub upper: usize,
    pub upper_at: f64,
    pub lower: usize,
    pub lower_at: f64,
}

pub fn decks_crossing(client: &mut Client) -> Result<Vec<DeckCrossing>, postgres::Error> {
    let rows = client.query(
        "SELECT u.id AS upper, ST_LineLocatePoint(u.geom, d.geom) * ST_Length(u.geom) AS upper_at,
                l.id AS lower, ST_LineLocatePoint(l.geom, d.geom) * ST_Length(l.geom) AS lower_at
         FROM deck_piece u
         JOIN deck_piece l ON l.geom && u.geom AND l.level < u.level AND ST_Intersects(l.geom, u.geom)
         CROSS JOIN LATERAL ST_Dump(ST_Intersection(u.geom, l.geom)) d
         WHERE GeometryType(d.geom) = 'POINT'",
        &[],
    )?;
    Ok(rows
        .iter()
        .map(|r| DeckCrossing {
            upper: r.get::<_, i32>("upper") as usize,
            upper_at: r.get("upper_at"),
            lower: r.get::<_, i32>("lower") as usize,
            lower_at: r.get("lower_at"),
        })
        .collect())
}

/// The ground at each point, in metres, sampled from the elevation raster
/// the pages' own is resampled from: zero where it has none, which is the
/// sea's.
pub fn ground(client: &mut Client, points: &[[f64; 2]]) -> Result<Vec<f64>, postgres::Error> {
    let mut out = vec![0.0; points.len()];
    for (b, batch) in points.chunks(BATCH).enumerate() {
        let xs: Vec<f64> = batch.iter().map(|p| p[0]).collect();
        let ys: Vec<f64> = batch.iter().map(|p| p[1]).collect();
        let rows = client.query(
            "SELECT DISTINCT ON (u.i) u.i, ST_Value(r.rast, 1, u.p, true, 'bilinear') AS z
             FROM (SELECT i, ST_SetSRID(ST_MakePoint(x, y), 3857) AS p FROM unnest($1::float8[], $2::float8[]) WITH ORDINALITY AS t(x, y, i)) u
             JOIN elevation_raster r ON ST_Intersects(r.rast, u.p)
             ORDER BY u.i",
            &[&xs, &ys],
        )?;
        for r in rows {
            let i = r.get::<_, i64>("i") as usize - 1;
            out[b * BATCH + i] = r.get::<_, Option<f64>>("z").unwrap_or(0.0);
        }
    }
    Ok(out)
}

/// A solved deck, as `overture_deck` holds it.
pub struct DeckOut<'a> {
    pub piece: &'a Piece,
    /// Each point, its top in metres as Z.
    pub points: Vec<[f64; 3]>,
    pub piers: Vec<u8>,
}

/// `overture_deck`, made again whole: the bake reads it a page at a time
/// (terrain/repository.rs).
pub fn write(client: &mut Client, decks: &[DeckOut]) -> Result<(), postgres::Error> {
    client.batch_execute(
        "DROP TABLE IF EXISTS overture_deck;
         CREATE TABLE overture_deck (gers_id TEXT NOT NULL, subtype TEXT NOT NULL, class TEXT, piers BYTEA NOT NULL, geom GEOMETRY(LineStringZ, 3857) NOT NULL);",
    )?;
    for batch in decks.chunks(BATCH) {
        let gers: Vec<&str> = batch.iter().map(|d| d.piece.gers_id.as_str()).collect();
        let subtypes: Vec<&str> = batch.iter().map(|d| d.piece.subtype.as_str()).collect();
        let classes: Vec<Option<&str>> = batch.iter().map(|d| d.piece.class.as_deref()).collect();
        let piers: Vec<&[u8]> = batch.iter().map(|d| d.piers.as_slice()).collect();
        let lines: Vec<String> = batch.iter().map(|d| wkt(d.points.iter().map(|q| format!("{} {} {:.2}", q[0], q[1], q[2])), true)).collect();
        client.execute(
            "INSERT INTO overture_deck SELECT g, s, c, p, ST_GeomFromText(w, 3857) FROM unnest($1::text[], $2::text[], $3::text[], $4::bytea[], $5::text[]) AS t(g, s, c, p, w)",
            &[&gers, &subtypes, &classes, &piers, &lines],
        )?;
    }
    client.batch_execute("CREATE INDEX ON overture_deck USING GIST (geom); ANALYZE overture_deck;")
}
