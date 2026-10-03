// pxl-geo-spec/table.rs

//! db-geo table names, for places that name a table without querying it.
//! SQL spells its table out so `grep` finds every reader.

pub const PLACES: &str = "overture_places";
pub const DIVISIONS: &str = "overture_divisions";
pub const DIVISION_POINTS: &str = "overture_division_points";
pub const TRANSPORTATION: &str = "overture_transportation";
pub const BASE_WATER: &str = "overture_base_water";
/// The deck solve's, made from `TRANSPORTATION` and `BASE_WATER`.
pub const DECK: &str = "overture_deck";

/// The one table an ingest appends to rather than replaces.
pub const CATALOG: &str = "overture_catalog";
