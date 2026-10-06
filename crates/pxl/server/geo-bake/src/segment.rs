// pxl-geo-bake/segment.rs

//! An Overture transportation segment as the bake reads it, for the ground
//! and the deck solve alike.

use crate::terrain::pipeline::overture::width::WayColumns;

/// The columns of `overture_transportation t` a [`Segment`] is read from.
#[macro_export]
macro_rules! segment_columns {
    () => {
        "t.gers_id, t.subtype, t.class, COALESCE(t.road_flags, t.rail_flags)::text AS flags, t.level_rules::text AS levels,
 t.subclass, t.subclass_rules::text AS subclasses,
    (t.properties->'connectors')::text AS connectors, ST_AsBinary(t.geom) AS geom_wkb"
    };
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Segment {
    pub gers_id: String,
    pub subtype: String,
    pub class: Option<String>,
    pub flags: Option<String>,
    pub levels: Option<String>,
    pub subclass: Option<String>,
    pub subclasses: Option<String>,
    /// Where along it it joins other segments.
    pub connectors: Option<String>,
    pub geom_wkb: Box<[u8]>,
}

impl Segment {
    pub fn of(r: &postgres::Row) -> Segment {
        Segment {
            gers_id: r.get("gers_id"),
            subtype: r.get("subtype"),
            class: r.get("class"),
            flags: r.get("flags"),
            levels: r.get("levels"),
            subclass: r.get("subclass"),
            subclasses: r.get("subclasses"),
            connectors: r.get("connectors"),
            geom_wkb: r.get::<_, &[u8]>("geom_wkb").into(),
        }
    }

    pub fn columns(&self) -> WayColumns<'_> {
        WayColumns {
            flags: self.flags.as_deref(),
            subclass: self.subclass.as_deref(),
            subclasses: self.subclasses.as_deref(),
        }
    }

    /// Bridged or tunnelled somewhere along it.
    pub fn flagged(&self) -> bool {
        self.flags.as_deref().is_some_and(|f| f.contains("is_bridge") || f.contains("is_tunnel"))
    }
}
