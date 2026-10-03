// pxl-geo-spec/table.rs

//! db-geo table names, for places that name a table without querying it.
//! SQL spells its table out so `grep` finds every reader.

/// The one table an ingest appends to rather than replaces.
pub const CATALOG: &str = "overture_catalog";
