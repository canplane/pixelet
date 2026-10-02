// pxl-geo-bake/terrain/leaf.rs

//! Which z16 leaves exist and their bytes. A leaf exists where the elevation
//! source covers it and land touches it; the offline bake and the dev server
//! ask the same question, so both produce the same objects.

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

use crate::terrain::artifact::assemble;
use crate::terrain::bake::bake_surface;
use crate::terrain::repository::TerrainRepository;
use crate::terrain::source::{fetch_lines, fetch_surface, SourceStats};
use crate::util::{pack, Level};

/// From `SELECT ST_Extent(ST_Transform(rast::geometry, 4326)) FROM elevation_raster`.
pub const SW: [f64; 2] = [123.9998611, 33.000244913560735];
pub const NE: [f64; 2] = [131.9997935265517, 39.000138899999996];

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

    pub fn meets(&self, level: u32, page: IVec) -> bool {
        let ([x0, y0], [x1, y1]) = (ancestor_of([self.x.0, self.y.0], level), ancestor_of([self.x.1, self.y.1], level));
        (x0..=x1).contains(&ivec::X(page)) && (y0..=y1).contains(&ivec::Y(page))
    }

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

/// A page with no land row is open water; a cheap check, not the carve.
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

pub fn bake_leaf(repo: &TerrainRepository, client: &mut Client, page: IVec, stats: &mut SourceStats, level: Level) -> Result<(usize, Vec<u8>), String> {
    let surface = fetch_surface(repo, client, page, stats).map_err(|e| e.to_string())?;
    let lines = fetch_lines(repo, client, page).map_err(|e| e.to_string())?;
    let raw = encode_terrain(&assemble(surface, lines)).map_err(|e| format!("{e:?}"))?;
    let packed = pack(raw.as_slice(), level).map_err(|e| e.to_string())?;
    Ok((raw.len(), packed))
}

/// Synthesized rather than read, so the ocean page does not depend on which
/// offshore page was sampled.
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
    }));
    encoder().encode(&root, TreeEncoderOptions::default()).expect("empty source packet")
}

/// Checked to be page-independent: readers keep one decoded copy, which is
/// sound only if the empty source bakes to identical bytes at any latitude.
pub fn bake_ocean(samples: &[IVec]) -> Result<Vec<u8>, String> {
    let src = empty_source();
    let baked: Vec<Vec<u8>> = samples
        .iter()
        .map(|p| bake_surface(*p, &src).and_then(|s| encode_terrain(&assemble(s, Default::default()))).expect("ocean bake"))
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
