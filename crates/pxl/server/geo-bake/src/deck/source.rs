// pxl-geo-bake/deck/source.rs

//! The solve's PostGIS queries over a staged, indexed `deck_piece` table,
//! fanned over several connections. Staging is unlogged, not temporary, so
//! every connection sees it.

use postgres::{Client, NoTls};

use crate::deck::piece::{BridgeRow, Piece};

const BATCH: usize = 20_000;

const LOOKUP: usize = 2_000;

const WORKERS: usize = 8;

/// A way within this of a point, in DB coordinates, passes through or ends there.
const TOUCH_REACH: f64 = 0.5;

/// A step along a way from a point, in DB coordinates.
pub const SIDE_STEP: f64 = 0.5;

pub fn fan<T: Send>(url: &str, ask: impl Fn(&mut Client, usize, usize) -> Result<Vec<T>, postgres::Error> + Sync) -> Result<Vec<T>, postgres::Error> {
    std::thread::scope(|scope| {
        let ask = &ask;
        let workers: Vec<_> = (0..WORKERS)
            .map(|k| {
                scope.spawn(move || {
                    let mut client = Client::connect(url, NoTls)?;
                    ask(&mut client, k, WORKERS)
                })
            })
            .collect();
        let mut out = Vec::new();
        for w in workers {
            out.extend(w.join().expect("solve worker")?);
        }
        Ok(out)
    })
}

fn share(k: usize, n: usize) -> (i32, i32) {
    (n as i32, k as i32)
}

pub fn flagged_rows(client: &mut Client, flag: &str) -> Result<Vec<BridgeRow>, postgres::Error> {
    let rows = client.query(
        "SELECT gers_id, subtype, class, COALESCE(road_flags, rail_flags)::text AS flags, level_rules::text AS levels,
                width_rules::text AS widths, access_restrictions::text AS access,
                subclass, subclass_rules::text AS subclasses, ST_AsBinary(geom) AS geom_wkb
         FROM overture_transportation
         WHERE COALESCE(road_flags, rail_flags)::text LIKE '%' || $1 || '%'",
        &[&flag],
    )?;
    Ok(rows
        .iter()
        .map(|r| BridgeRow {
            gers_id: r.get("gers_id"),
            subtype: r.get("subtype"),
            class: r.get("class"),
            flags: r.get("flags"),
            levels: r.get("levels"),
            widths: r.get("widths"),
            access: r.get("access"),
            subclass: r.get("subclass"),
            subclasses: r.get("subclasses"),
            geom_wkb: r.get::<_, &[u8]>("geom_wkb").into(),
        })
        .collect())
}

fn wkt(points: impl Iterator<Item = String>, z: bool) -> String {
    format!("LINESTRING{} ({})", if z { " Z" } else { "" }, points.collect::<Vec<_>>().join(","))
}

pub fn stage(client: &mut Client, pieces: &[&Piece]) -> Result<(), postgres::Error> {
    client.batch_execute(
        "DROP TABLE IF EXISTS deck_piece;
         CREATE UNLOGGED TABLE deck_piece (id INT PRIMARY KEY, gers_id TEXT NOT NULL, level INT NOT NULL, geom GEOMETRY(LineString, 3857) NOT NULL);",
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

pub struct WayCrossing {
    pub piece: usize,
    pub at: f64,
    pub subtype: String,
    pub class: Option<String>,
    pub flags: Option<String>,
    pub own_at: f64,
}

pub fn ways_crossing(url: &str) -> Result<Vec<WayCrossing>, postgres::Error> {
    fan(url, |client, k, n| {
        let (n, k) = share(k, n);
        let rows = client.query(
            "SELECT p.id, ST_LineLocatePoint(p.geom, d.geom) * ST_Length(p.geom) AS at,
                t.subtype, t.class, COALESCE(t.road_flags, t.rail_flags)::text AS flags,
                ST_LineLocatePoint(t.geom, d.geom) AS own_at
         FROM deck_piece p
         JOIN overture_transportation t ON t.geom && p.geom AND t.gers_id <> p.gers_id AND ST_Intersects(t.geom, p.geom)
         CROSS JOIN LATERAL ST_Dump(ST_Intersection(t.geom, p.geom)) d
         WHERE p.id % $1 = $2 AND GeometryType(d.geom) = 'POINT'",
            &[&n, &k],
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
    })
}

pub struct WaterUnder {
    pub piece: usize,
    pub from: f64,
    pub to: f64,
    pub subtype: String,
    pub class: Option<String>,
}

/// The water is subdivided first: one river polygon reaches 440,000 points
/// over a province's box. A deck on a seam is under both pieces.
pub fn water_under(client: &mut Client, url: &str) -> Result<Vec<WaterUnder>, postgres::Error> {
    client.batch_execute(
        "DROP TABLE IF EXISTS deck_water;
         CREATE UNLOGGED TABLE deck_water AS
           SELECT w.subtype, w.class, ST_Subdivide(w.geom, 256) AS geom
           FROM overture_base_water w
           WHERE w.subtype <> 'ocean' AND EXISTS (SELECT 1 FROM deck_piece p WHERE p.geom && w.geom);
         CREATE INDEX ON deck_water USING GIST (geom);
         ANALYZE deck_water;",
    )?;
    fan(url, |client, k, n| {
        let (n, k) = share(k, n);
        let rows = client.query(
            "SELECT p.id,
                ST_LineLocatePoint(p.geom, COALESCE(ST_StartPoint(d.geom), d.geom)) * ST_Length(p.geom) AS a,
                ST_LineLocatePoint(p.geom, COALESCE(ST_EndPoint(d.geom), d.geom)) * ST_Length(p.geom) AS b,
                w.subtype, w.class
         FROM deck_piece p
         JOIN deck_water w ON w.geom && p.geom AND ST_Intersects(w.geom, p.geom)
         CROSS JOIN LATERAL ST_Dump(ST_Intersection(p.geom, w.geom)) d
         WHERE p.id % $1 = $2 AND GeometryType(d.geom) IN ('POINT', 'LINESTRING')",
            &[&n, &k],
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
    })
}

/// Two staged pieces crossing through each other, not meeting at an end:
/// `upper` over `lower` by `layer`, and where the layers tie, by id, so the
/// order is one total order.
pub struct DeckCrossing {
    pub upper: usize,
    pub upper_at: f64,
    pub lower: usize,
    pub lower_at: f64,
    pub tied: bool,
}

pub fn decks_crossing(url: &str) -> Result<Vec<DeckCrossing>, postgres::Error> {
    fan(url, |client, k, n| {
        let (n, k) = share(k, n);
        let rows = client.query(
            "SELECT u.id AS upper, ST_LineLocatePoint(u.geom, d.geom) * ST_Length(u.geom) AS upper_at,
                l.id AS lower, ST_LineLocatePoint(l.geom, d.geom) * ST_Length(l.geom) AS lower_at, l.level = u.level AS tied
         FROM deck_piece u
         JOIN deck_piece l ON l.geom && u.geom AND (l.level, l.gers_id) < (u.level, u.gers_id) AND ST_Crosses(l.geom, u.geom)
         CROSS JOIN LATERAL ST_Dump(ST_Intersection(u.geom, l.geom)) d
         WHERE u.id % $1 = $2 AND GeometryType(d.geom) = 'POINT'",
            &[&n, &k],
        )?;
        Ok(rows
            .iter()
            .map(|r| DeckCrossing {
                upper: r.get::<_, i32>("upper") as usize,
                upper_at: r.get("upper_at"),
                lower: r.get::<_, i32>("lower") as usize,
                lower_at: r.get("lower_at"),
                tied: r.get("tied"),
            })
            .collect())
    })
}

/// Ways connected at each point (ending there, or the point's own segment),
/// not ones passing over or under, and where along each the point is.
pub fn connected_rows(url: &str, points: &[([f64; 2], &str)]) -> Result<Vec<(usize, f64, BridgeRow)>, postgres::Error> {
    fan(url, |client, k, n| {
        let mut out = Vec::new();
        for (b, batch) in points.chunks(LOOKUP).enumerate().filter(|(b, _)| b % n == k) {
            let xs: Vec<f64> = batch.iter().map(|p| p.0[0]).collect();
            let ys: Vec<f64> = batch.iter().map(|p| p.0[1]).collect();
            let gers: Vec<&str> = batch.iter().map(|p| p.1).collect();
            let rows = client.query(
                "SELECT u.i, ST_LineLocatePoint(t.geom, u.p) AS own_at, t.gers_id, t.subtype, t.class,
                    COALESCE(t.road_flags, t.rail_flags)::text AS flags, t.level_rules::text AS levels,
                    t.width_rules::text AS widths, t.access_restrictions::text AS access,
                    t.subclass, t.subclass_rules::text AS subclasses, ST_AsBinary(t.geom) AS geom_wkb
             FROM (SELECT i, g, ST_SetSRID(ST_MakePoint(x, y), 3857) AS p FROM unnest($1::float8[], $2::float8[], $3::text[]) WITH ORDINALITY AS t(x, y, g, i)) u
             JOIN overture_transportation t ON t.geom && ST_Expand(u.p, $4::float8) AND ST_DWithin(t.geom, u.p, $4::float8)
               AND (t.gers_id = u.g OR ST_DWithin(ST_StartPoint(t.geom), u.p, $4::float8) OR ST_DWithin(ST_EndPoint(t.geom), u.p, $4::float8))",
                &[&xs, &ys, &gers, &TOUCH_REACH],
            )?;
            for r in rows {
                out.push((
                    b * LOOKUP + r.get::<_, i64>("i") as usize - 1,
                    r.get("own_at"),
                    BridgeRow {
                        gers_id: r.get("gers_id"),
                        subtype: r.get("subtype"),
                        class: r.get("class"),
                        flags: r.get("flags"),
                        levels: r.get("levels"),
                        widths: r.get("widths"),
                        access: r.get("access"),
                        subclass: r.get("subclass"),
                        subclasses: r.get("subclasses"),
                        geom_wkb: r.get::<_, &[u8]>("geom_wkb").into(),
                    },
                ));
            }
        }
        Ok(out)
    })
}

pub struct DeckOut<'a> {
    pub piece: &'a Piece,
    pub points: Vec<[f64; 3]>,
}

pub fn write(client: &mut Client, decks: &[DeckOut]) -> Result<(), postgres::Error> {
    client.batch_execute(
        "DROP TABLE IF EXISTS overture_deck;
         CREATE TABLE overture_deck (gers_id TEXT NOT NULL, subtype TEXT NOT NULL, class TEXT, width_m REAL NOT NULL, form SMALLINT NOT NULL, geom GEOMETRY(LineStringZ, 3857) NOT NULL);",
    )?;
    for batch in decks.chunks(BATCH) {
        let gers: Vec<&str> = batch.iter().map(|d| d.piece.gers_id.as_str()).collect();
        let subtypes: Vec<&str> = batch.iter().map(|d| d.piece.subtype.as_str()).collect();
        let classes: Vec<Option<&str>> = batch.iter().map(|d| d.piece.class.as_deref()).collect();
        let widths: Vec<f32> = batch.iter().map(|d| d.piece.width_m as f32).collect();
        let forms: Vec<i16> = batch.iter().map(|d| i16::from(d.piece.form.byte())).collect();
        let lines: Vec<String> = batch.iter().map(|d| wkt(d.points.iter().map(|q| format!("{} {} {:.2}", q[0], q[1], q[2])), true)).collect();
        client.execute(
            "INSERT INTO overture_deck SELECT g, s, c, m, f, ST_GeomFromText(w, 3857) FROM unnest($1::text[], $2::text[], $3::text[], $4::real[], $5::int2[], $6::text[]) AS t(g, s, c, m, f, w)",
            &[&gers, &subtypes, &classes, &widths, &forms, &lines],
        )?;
    }
    client.batch_execute("CREATE INDEX ON overture_deck USING GIST (geom); ANALYZE overture_deck; DROP TABLE IF EXISTS deck_piece, deck_water;")
}
