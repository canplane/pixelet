// pxl-geo-bake/terrain/bake.rs

//! One page's source rows baked into its surface, which with its lines is the page.

use pxl_spec::bake::deck::Decks;
use pxl_spec::bake::format::{Ground, Terrain};
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::IVec;

use crate::deck::Carried;
use crate::terrain::artifact::Surface;
use crate::terrain::pipeline::coast::adapter::from_coast_row;
use crate::terrain::pipeline::coast::policy::CoastPolicy;
use crate::terrain::pipeline::corridor::corridor_fills;
use crate::terrain::pipeline::feature::{local, Feature};
use crate::terrain::pipeline::ground::{bake_ground, Coast};
use crate::terrain::pipeline::overture::policy::{bases, buildings, ways};
use crate::terrain::repository::PageSource;
use crate::wkb::wkb_to_geometries;

/// `carried`: where the deck solve carries the page's ways off the ground.
pub fn bake_surface(page_idx: IVec, ele: Vec<i16>, source: &PageSource, carried: &[Carried]) -> Surface {
    let page_min = epsg3857::get_page_bbox(page_idx)[0];
    let mut feats: Vec<Feature> = Vec::new();

    for i in 0..buildings.len {
        for raw in from_building_row(&building_row(&buildings.at_branch(i)?)?) {
            feats.extend(normalize(&raw, page_min, &OverturePolicy));
        }
    }
    for row in &source.segment_rows {
        feats.extend(from_segment_row(row, carried).iter().filter_map(|raw| normalize(raw, page_min, &OverturePolicy)));
    }
    for i in 0..base.len {
        for raw in from_base_row(&base_row(&base.at_branch(i)?)?) {
            feats.extend(normalize(&raw, page_min, &OverturePolicy));
        }
    }
    for i in 0..coasts.len {
        for raw in from_coast_row(&coast_row(&coasts.at_branch(i)?)?) {
            feats.extend(normalize(&raw, page_min, &CoastPolicy));
        }
    }

    // Whether *any* coastline row touched this page's envelope at all, even if
    // from_coast_row dropped it (covers_page => full land). bake_ground needs
    // this to tell "no land data at all (open ocean)" apart from "land data
    // present but not worth rasterizing (full land)".
    let has_coast_row = coasts.len > 0;

    let ground: Ground = bake_ground(&feats, has_coast_row);

    let rows = (0..deck_rows.len).map(|i| deck_row(&deck_rows.at_branch(i)?)).collect::<Result<Vec<_>, _>>()?;
    let decks = bake_decks(&rows, page_min).ok_or(MarshalError::new(Errno::ENOSPC))?;

    Ok(Terrain::leaf(ele.to_vec(), ground, decks))
}
