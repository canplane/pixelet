// pxl-geo-spec/table.rs

//! db-geo table names, for places that name a table without querying it.
//! SQL spells its table out so `grep` finds every reader.

pub const PLACES: &str = "overture_places";
pub const DIVISIONS: &str = "overture_divisions";
pub const DIVISION_POINTS: &str = "overture_division_points";

/// The one table an ingest appends to rather than replaces.
pub const CATALOG: &str = "overture_catalog";
