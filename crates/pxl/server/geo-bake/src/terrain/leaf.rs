// pxl-geo-bake/terrain/leaf.rs

//! Which z16 leaves exist and their bytes. A leaf exists where the elevation
//! source covers it and land touches it; the offline bake and the dev server
//! ask the same question, so both produce the same objects.

use std::path::Path;

use postgres::types::ToSql;
use postgres::Client;
use pxl_spec::bake::format::{decode_terrain, encode_terrain, Occupancy, Terrain, ELE_LANE};
use pxl_spec::space::adapter::epsg3857::{self, to_db};
use pxl_spec::space::indexer::L3;
use pxl_spec::space::pvf3::ivec::{self, IVec};

use crate::coverage::Coverage;
use crate::deck::{self, source::Store};
use crate::terrain::bake::{assemble, bake_surface};
use crate::terrain::pipeline::deck::bake_decks;
use crate::terrain::repository::{self, PageSource, DRAWN_REACH_U};
use crate::terrain::source::fetch_surface;
use crate::util::fnv1a;

pub use pxl_spec::bake::format::OCEAN_KEY;

/// Whether anything is drawn in a page: land or a building in it, or a way
/// (not a ferry) within `DRAWN_REACH_U`. `CASE` fixes the order, which
/// `OR` does not, so the costly coastline test runs last.
const HAS_GROUND_SQL: &str = "
  SELECT t.ord, CASE
    WHEN EXISTS (SELECT 1 FROM overture_buildings b WHERE ST_Intersects(b.geom, t.page)) THEN true
    WHEN EXISTS (SELECT 1 FROM overture_transportation s WHERE ST_DWithin(s.geom, t.page, $6) AND s.subtype <> 'water') THEN true
    ELSE EXISTS (SELECT 1 FROM overture_base_land c WHERE ST_Intersects(c.geom, t.page))
  END AS has_ground
  FROM (
    SELECT ord, ST_MakeEnvelope(minx, miny, maxx, maxy, 3857) AS page
    FROM unnest($1::int[], $2::float8[], $3::float8[], $4::float8[], $5::float8[]) AS u(ord, minx, miny, maxx, maxy)
  ) t
";

/// What classifying reads; with a page's reads, all a terrain bake reads.
pub const GROUND_READS: [&str; 1] = [HAS_GROUND_SQL];
pub const READS: [&[&str]; 3] = [&GROUND_READS, &repository::READS, &deck::source::READS];

/// The rule a land list was classified by: a list from another is stale.
const GROUND: u64 = fnv1a(HAS_GROUND_SQL.as_bytes()) ^ DRAWN_REACH_U.to_bits();

/// A page nothing is drawn in is open water; a cheap check, not the carve.
pub fn has_ground(client: &mut Client, pages: &[IVec]) -> Result<Vec<bool>, postgres::Error> {
    let bbox: Vec<[f64; 4]> = pages
        .iter()
        .map(|&p| {
            let bb = epsg3857::get_page_bbox(p);
            [bb[0][0], bb[0][1], bb[1][0], bb[1][1]]
        })
        .collect();
    let ord: Vec<i32> = (0..pages.len() as i32).collect();
    let col = |k: usize| -> Vec<f64> { bbox.iter().map(|b| b[k]).collect() };
    let (minx, miny, maxx, maxy) = (col(0), col(1), col(2), col(3));
    let reach = to_db(DRAWN_REACH_U);
    let params: [&(dyn ToSql + Sync); 6] = [&ord, &minx, &miny, &maxx, &maxy, &reach];

    let mut out = vec![false; pages.len()];
    for row in client.query(HAS_GROUND_SQL, &params)? {
        out[row.get::<_, i32>("ord") as usize] = row.get::<_, bool>("has_ground");
    }
    Ok(out)
}

/// A leaf: its decks solved, its surface painted around their lines, the two
/// one page; and whether it lies off the elevation raster, so flat.
pub fn bake_leaf(r: &mut deck::Reader<impl Store>, page: IVec) -> Result<(Terrain, bool), String> {
    let solved = deck::solve_page(r, page)?;
    let (surface, flat) = fetch_surface(r.repo, r.client, page, &solved.drawn)?;
    let lines = bake_decks(&solved.lines, epsg3857::get_page_bbox(page)[0]).ok_or("a page's decks overflow a record")?;
    Ok((assemble(surface, lines), flat))
}

/// Synthesized rather than read, so the ocean page does not depend on which
/// offshore page was sampled.
fn empty_source() -> PageSource {
    PageSource {
        ele: None,
        building_rows: Vec::new(),
        segment_rows: Vec::new(),
        base_rows: Vec::new(),
        coast_rows: Vec::new(),
    }
}

/// Checked to be page-independent: readers keep one decoded copy, which is
/// sound only if the empty source bakes to identical bytes at any latitude.
pub fn bake_ocean(samples: &[IVec]) -> Result<Vec<u8>, String> {
    let src = empty_source();
    let baked: Vec<Vec<u8>> = samples
        .iter()
        .map(|p| encode_terrain(&assemble(bake_surface(*p, vec![0; ELE_LANE], &src, &[]), Default::default())).expect("ocean bake"))
        .collect();
    for (i, page) in baked.iter().enumerate().skip(1) {
        if page != &baked[0] {
            return Err(format!(
                "the ocean page is NOT page-independent: page {} and {} baked to different bytes ({} vs {}). \
                 The shared-fallback plan does not hold; ocean pages would each need their own object.",
                ivec::repr(samples[0]),
                ivec::repr(samples[i]),
                baked[0].len(),
                page.len()
            ));
        }
    }
    let mut ocean = decode_terrain(&baked[0]).map_err(|e| format!("{e:?}"))?;
    ocean.index.occupancy = Occupancy::ZERO;
    encode_terrain(&ocean).map_err(|e| format!("{e:?}"))
}

pub fn ocean_samples(coverage: &Coverage) -> [IVec; 3] {
    let mid = (coverage.x.0 + coverage.x.1) / 2;
    [
        ivec::gather(coverage.x.0, coverage.y.0, 0),
        ivec::gather(mid, (coverage.y.0 + coverage.y.1) / 2, 0),
        ivec::gather(coverage.x.1, coverage.y.1, 0),
    ]
}

/// The pages `has_ground` found, kept for the geocode bake and later stages;
/// refused unless made at this page size, coverage, rule and release.
pub fn land_pages(path: &Path, coverage: &Coverage, release: Option<&str>) -> Result<Vec<IVec>, String> {
    let reclassify = "run bake_tiles, which writes it as a side effect of its own classification (`--limit 0` does the classification and bakes nothing)";
    let raw = std::fs::read_to_string(path).map_err(|_| format!("{} not found — {reclassify}.", path.display()))?;
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let want = land_key(coverage, release.unwrap_or_default());
    for (k, want) in want.as_object().into_iter().flatten() {
        if (k != "release" || release.is_some()) && v.get(k) != Some(want) {
            return Err(format!("{} was classified with {k} {}, and it is now {want} — {reclassify}.", path.display(), v.get(k).unwrap_or(&serde_json::Value::Null)));
        }
    }
    let list = v.get("pages").and_then(|p| p.as_array()).ok_or_else(|| format!("{} has no page list", path.display()))?;
    list.iter().map(|s| s.as_str().ok_or("a page is not a string")?.parse::<IVec>().map_err(|e| e.to_string())).collect()
}

pub fn write_land_pages(path: &Path, coverage: &Coverage, release: &str, land: &[IVec]) -> Result<(), String> {
    let mut v = land_key(coverage, release);
    v["pages"] = land.iter().map(|p| p.to_string()).collect();
    std::fs::write(path, serde_json::to_string(&v).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn land_key(coverage: &Coverage, release: &str) -> serde_json::Value {
    serde_json::json!({
        "l3_shamt": L3.shamt(),
        "x": [coverage.x.0, coverage.x.1],
        "y": [coverage.y.0, coverage.y.1],
        "ground": format!("{GROUND:016x}"),
        "release": release,
    })
}
