// pxl-geo-bake/terrain/leaf.rs

//! The leaves: which z16 pages exist, and each one's bytes.
//!
//! A leaf exists where the elevation source covers it and land touches it.
//! Anything else is open water, which the bake leaves as no object and every
//! reader resolves to the one shared ocean page. The offline bake asks this of
//! every page in the coverage at once; the dev tile server asks it of one page
//! when a page is first wanted. It is the same question and the same answer, so
//! the two produce the same set of objects, byte for byte.

use postgres::types::ToSql;
use postgres::Client;
use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::bake::format::{decode_terrain, encode_terrain, Occupancy};
use pxl_spec::space::adapter::{epsg3857, epsg4326};
use pxl_spec::space::hierarchy::ancestor_of;
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use xpute_core::wire::xtp::{encoder, BranchView, NodeView, TreeEncoderOptions, TreeView};

use crate::terrain::bake::bake_terrain;
use crate::terrain::repository::TerrainRepository;
use crate::terrain::source::{fetch_tile, SourceStats};
use crate::util::{pack, Level};

/// Current `elevation_raster` coverage bbox — confirmed with
/// `SELECT ST_Extent(ST_Transform(rast::geometry, 4326)) FROM elevation_raster`.
pub const SW: [f64; 2] = [123.9998611, 33.000244913560735];
pub const NE: [f64; 2] = [131.9997935265517, 39.000138899999996];

/// Object key of the single fallback page every open-water page resolves to.
pub const OCEAN_KEY: &str = "_ocean";

const HAS_LAND_SQL: &str = "
  SELECT t.ord, EXISTS (
    SELECT 1 FROM overture_base_land c
    WHERE c.geom && ST_MakeEnvelope(t.minx, t.miny, t.maxx, t.maxy, 3857)
      AND ST_Intersects(c.geom, ST_MakeEnvelope(t.minx, t.miny, t.maxx, t.maxy, 3857))
  ) AS has_land
  FROM unnest($1::int[], $2::float8[], $3::float8[], $4::float8[], $5::float8[])
    AS t(ord, minx, miny, maxx, maxy)
";

/// The leaves the elevation source covers, as an inclusive page range.
#[derive(Clone, Copy, Debug)]
pub struct Coverage {
    pub x: (i64, i64),
    pub y: (i64, i64),
}

fn project_page(lng: f64, lat: f64) -> IVec {
    let mut p = Pvf3 { i: 0, f: 0 };
    epsg4326::project(&mut p, &[lng, lat, 0.0]);
    L3_IDX(p.i)
}

impl Coverage {
    pub fn of_source() -> Coverage {
        let (sw, ne) = (project_page(SW[0], SW[1]), project_page(NE[0], NE[1]));
        let (x1, x2) = (ivec::X(sw), ivec::X(ne));
        let (y1, y2) = (ivec::Y(sw), ivec::Y(ne));
        Coverage {
            x: (x1.min(x2), x1.max(x2)),
            y: (y1.min(y2), y1.max(y2)),
        }
    }

    pub fn contains(&self, page: IVec) -> bool {
        (self.x.0..=self.x.1).contains(&ivec::X(page)) && (self.y.0..=self.y.1).contains(&ivec::Y(page))
    }

    /// Whether a level-`level` page's footprint — the 2^level leaves a side it
    /// covers — meets the range. A page that does not can have no leaf under
    /// it, so it does not exist whatever else is asked.
    pub fn meets(&self, level: u32, page: IVec) -> bool {
        let ([x0, y0], [x1, y1]) = (ancestor_of([self.x.0, self.y.0], level), ancestor_of([self.x.1, self.y.1], level));
        (x0..=x1).contains(&ivec::X(page)) && (y0..=y1).contains(&ivec::Y(page))
    }

    /// Every page in the range, X-major.
    pub fn pages(&self) -> Vec<IVec> {
        let mut out = Vec::new();
        for x in self.x.0..=self.x.1 {
            for y in self.y.0..=self.y.1 {
                out.push(ivec::gather(x, y, 0));
            }
        }
        out
    }
}

/// Which of `pages` land touches, in their order. A page with no land row is
/// open water, whatever the coastline carve would later make of it: this is
/// the cheap existence check, not the bake.
pub fn has_land(client: &mut Client, pages: &[IVec]) -> Result<Vec<bool>, postgres::Error> {
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
    let params: [&(dyn ToSql + Sync); 5] = [&ord, &minx, &miny, &maxx, &maxy];

    let mut out = vec![false; pages.len()];
    for row in client.query(HAS_LAND_SQL, &params)? {
        out[row.get::<_, i32>("ord") as usize] = row.get::<_, bool>("has_land");
    }
    Ok(out)
}

/// A leaf's object, packed as it is stored and served, and its raw size.
pub fn bake_leaf(repo: &TerrainRepository, client: &mut Client, page: IVec, stats: &mut SourceStats, level: Level) -> Result<(usize, Vec<u8>), String> {
    let raw = fetch_tile(repo, client, page, stats).map_err(|e| e.to_string())?;
    let packed = pack(raw.as_slice(), level).map_err(|e| e.to_string())?;
    Ok((raw.len(), packed))
}

/// The source packet an open-ocean page would produce: a flat zero elevation
/// grid, no terrain features, no coastline rows. Synthesized rather than read
/// from the DB so the fallback page is exactly "nothing here but water",
/// independent of whichever far-offshore page happened to get sampled.
fn empty_source() -> Vec<u8> {
    let ele = vec![0i16; (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize];
    let mut empty: TreeView<'_> = TreeView::new();
    empty.branch(Some(&mut |_| {}));

    let mut root: TreeView<'_> = TreeView::new();
    root.branch(Some(&mut |r| {
        r.i16_array(Some(&ele));
        r.subtree(&empty); // buildings
        r.subtree(&empty); // segments
        r.subtree(&empty); // base
        r.subtree(&empty); // coasts
        r.subtree(&empty); // decks
    }));
    encoder().encode(&root, TreeEncoderOptions::default()).expect("empty source packet")
}

/// The ocean page, raw, checked to be page-independent: a reader keeps one
/// decoded copy for every ocean page, which is only sound if baking the same
/// empty source at different pages (different bbox, different `Z_SCALE` by
/// latitude) yields identical bytes.
pub fn bake_ocean(samples: &[IVec]) -> Result<Vec<u8>, String> {
    let src = empty_source();
    let baked: Vec<Vec<u8>> = samples.iter().map(|p| bake_terrain(*p, &src).and_then(|t| encode_terrain(&t)).expect("ocean bake")).collect();
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
    // Baked as a leaf, but it is the page every page with no object reads:
    // the zero page, which is no page of its own (pxl-spec bake/format.rs
    // `Occupancy`), and which a parent over it says so of.
    let mut ocean = decode_terrain(&baked[0]).map_err(|e| format!("{e:?}"))?;
    ocean.index.occupancy = Occupancy::ZERO;
    encode_terrain(&ocean).map_err(|e| format!("{e:?}"))
}

/// Three pages far apart in latitude, which is what the ocean check compares.
pub fn ocean_samples(coverage: &Coverage) -> [IVec; 3] {
    let mid = (coverage.x.0 + coverage.x.1) / 2;
    [
        ivec::gather(coverage.x.0, coverage.y.0, 0),
        ivec::gather(mid, (coverage.y.0 + coverage.y.1) / 2, 0),
        ivec::gather(coverage.x.1, coverage.y.1, 0),
    ]
}
