// pxl-geo-spec/table.rs

//! The tables in db-geo, by name.
//!
//! Three sides know each of these: the ingest that creates it
//! (infra/geo/overture/script/), the live server that queries it, and the bake
//! that walks it.
//!
//! **A query spells its table out rather than interpolating this.** The name
//! in the SQL is how anyone finds who reads a table — `grep overture_places`
//! answers "what queries this" in one line, and a format string answers
//! nothing. A name that drifted would fail at `prepare` on the first run
//! either way, loudly and locally, which is not the failure a constant is for.
//! What is here is for the places that *name* a table without querying it: a
//! manifest saying which tables a bake read, a catalog lookup keyed by name.
//!
//! What each table holds is not here. A column list belongs to the statement
//! that reads it, and the two readers ask for different columns of the same
//! table.

pub const PLACES: &str = "overture_places";
pub const DIVISIONS: &str = "overture_divisions";
pub const DIVISION_POINTS: &str = "overture_division_points";

/// Which release each of the above was pulled from, and with what bbox. The
/// one table an ingest adds to rather than replacing.
pub const CATALOG: &str = "overture_catalog";
