// pxl-geo-bake/deck.rs

//! Decks and bores: what a tile's bridges and tunnels are, read from the
//! source, and the lines the bake stores for them, solved as Arnis solves
//! them (`deck/arnis.rs`).

pub mod arnis;
pub mod elevation;
pub mod piece;
pub mod source;
pub mod way;

use postgres::Client;
use pxl_spec::bake::deck::Form;
use pxl_spec::space::adapter::epsg3857::G3857;

use crate::terrain::repository::TerrainRepository;
use source::Store;

pub use arnis::solve_tile;

/// A way's parts these lift onto a deck or sink into a bore.
pub const DECK_FLAGS: &[&str] = &["is_bridge"];
pub const BORE_FLAGS: &[&str] = &["is_tunnel"];

/// The flags that keep a way's part out of a form, as Arnis keeps it: no
/// indoor bridge (`is_bridge_way`), no road tunnel indoors or under
/// construction (`renders_as_highway_tunnel`), no rail bridge or tunnel on
/// a line abandoned or unbuilt (`RAIL_TRACK_TYPES`, `renders_as_rail_bridge`).
pub fn barred(form: Form, subtype: &str) -> &'static [&'static str] {
    match (form, subtype == "rail") {
        (Form::Deck, false) => &["is_tunnel", "is_indoor"],
        (Form::Deck, true) => &["is_tunnel", "is_indoor", "is_under_construction", "is_abandoned"],
        (_, false) => &["is_indoor", "is_under_construction"],
        (_, true) => &["is_under_construction", "is_abandoned"],
    }
}

/// What the solve leaves a tile: its lines, and the ranges of ways they draw,
/// which the ground leaves unpainted. One solve makes both, so the road
/// leaves the ground's paint exactly where its line begins.
#[derive(Default)]
pub struct Solved {
    pub lines: Vec<Line>,
    pub drawn: Vec<Drawn>,
}

/// A solved line: XY in DB coordinates, height and width in leaf cells.
pub struct Line {
    pub subtype: String,
    pub class: Option<String>,
    pub width_cells: f64,
    pub form: Form,
    pub points: Vec<G3857>,
    /// A bore's clear space over its surface, in leaf cells; none for another form.
    pub clear: u8,
}

/// A range of a way, as fractions of it, that a line draws.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawn {
    pub gers_id: String,
    pub range: [f64; 2],
}

/// What a solve reads through: the DB, and each tile's rows.
pub struct Reader<'a, S: Store> {
    pub client: &'a mut Client,
    pub repo: &'a TerrainRepository,
    pub store: &'a mut S,
}
