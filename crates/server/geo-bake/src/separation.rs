// pxl-geo-bake/separation.rs

//! Bridges and tunnels: what a tile's bridges and tunnels are, read from the
//! source, and the lines the bake stores for them, solved as Arnis solves
//! them (`separation/arnis.rs`).

pub mod arnis;
pub mod elevation;
pub mod piece;
pub mod source;
pub mod way;

use postgres::Client;
use pxl_spec::bake::separation::Form;
use pxl_spec::space::coord::adapter::epsg3857::G3857;

use crate::terrain::repository::TerrainRepository;
use source::Store;

pub use arnis::solve_tile;

/// What the solve leaves a tile: its lines, and the ranges of ways they draw,
/// which the ground leaves unpainted. One solve makes both, so the road
/// leaves the ground's paint exactly where its line begins.
#[derive(Default)]
pub struct Solved {
    pub lines: Vec<Line>,
    pub drawn: Vec<Drawn>,
}

/// A solved line: XY in DB coordinates, height in leaf cells, width in u.
pub struct Line {
    pub subtype: String,
    pub class: Option<String>,
    pub width_u: f64,
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
