// pxl-geo-bake/terrain/bake.rs

//! One page's source rows baked into its surface, which with its lines is the page.

use pxl_spec::bake::deck::Decks;
use pxl_spec::bake::format::{Ground, Terrain};
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::IVec;

use crate::deck::Drawn;
use crate::terrain::pipeline::corridor::corridor_fills;
use crate::terrain::pipeline::feature::{local, Feature};
use crate::terrain::pipeline::ground::{bake_ground, Coast};
use crate::terrain::pipeline::overture::policy::{bases, buildings, ways};
use crate::terrain::repository::PageSource;
use crate::wkb::wkb_to_geometries;

pub struct Surface {
    pub ele: Vec<i16>,
    pub ground: Ground,
}

pub fn assemble(surface: Surface, lines: Decks) -> Terrain {
    Terrain::leaf(surface.ele, surface.ground, lines)
}

fn building_row(row: &BranchCursor<'_>) -> Result<BuildingRow, MarshalError> {
    Ok(BuildingRow {
        gers_id: row.at(0)?.get_str()?,
        subtype: opt_str(row, 1)?,
        class: opt_str(row, 2)?,
        height: opt_num(row, 3)?,
        num_floors: opt_num(row, 4)?,
        geom_wkb: row.at(5)?.get_array::<u8>()?.into(),
    })
}

fn segment_row(row: &BranchCursor<'_>) -> Result<SegmentRow, MarshalError> {
    Ok(SegmentRow {
        gers_id: row.at(0)?.get_str()?,
        subtype: row.at(1)?.get_str()?,
        class: opt_str(row, 2)?,
        flags: opt_str(row, 3)?,
        widths: opt_str(row, 4)?,
        access: opt_str(row, 5)?,
        subclass: opt_str(row, 6)?,
        subclasses: opt_str(row, 7)?,
        geom_wkb: row.at(8)?.get_array::<u8>()?.into(),
    })
}

fn deck_row(row: &BranchCursor<'_>) -> Result<DeckRow, MarshalError> {
    Ok(DeckRow {
        gers_id: row.at(0)?.get_str()?,
        subtype: row.at(1)?.get_str()?,
        class: opt_str(row, 2)?,
        width_m: row.at(3)?.get_number()?,
        piers: row.at(4)?.get_array::<u8>()?.into(),
        geom_wkb: row.at(5)?.get_array::<u8>()?.into(),
    })
}

fn base_row(row: &BranchCursor<'_>) -> Result<BaseRow, MarshalError> {
    Ok(BaseRow {
        gers_id: row.at(0)?.get_str()?,
        source: row.at(1)?.get_str()?,
        subtype: opt_str(row, 2)?,
        class: opt_str(row, 3)?,
        geom_wkb: row.at(4)?.get_array::<u8>()?.into(),
    })
}

fn coast_row(row: &BranchCursor<'_>) -> Result<CoastRow, MarshalError> {
    Ok(CoastRow {
        id: row.at(0)?.get_str()?,
        geom_wkb: row.at(1)?.get_array::<u8>()?.into(),
        covers_page: row.at(2)?.get_bool()?,
    })
}

pub fn bake_terrain(page_idx: IVec, pkt: &[u8]) -> Result<Terrain, MarshalError> {
    let page_bbox = epsg3857::get_page_bbox(page_idx);
    let page_min = page_bbox[0];

    let root = TreeReader::new(pkt)?.read_branch()?;

    let ele: &[i16] = root.at(0)?.get_array::<i16>()?;
    let buildings = root.at(1)?.as_branch()?;
    let segments = root.at(2)?.as_branch()?;
    let base = root.at(3)?.as_branch()?;
    let coasts = root.at(4)?.as_branch()?;
    let deck_rows = root.at(5)?.as_branch()?;

    let mut feats: Vec<Feature> = Vec::new();

    for i in 0..buildings.len {
        for raw in from_building_row(&building_row(&buildings.at_branch(i)?)?) {
            feats.extend(normalize(&raw, page_min, &OverturePolicy));
        }
    }
    for i in 0..segments.len {
        for raw in from_segment_row(&segment_row(&segments.at_branch(i)?)?) {
            feats.extend(normalize(&raw, page_min, &OverturePolicy));
        }
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
