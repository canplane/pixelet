// pxl-geo-bake/deck/memo.rs

//! A solved structure, kept so the pages it crosses solve it once. Nothing
//! reads it as a fact: a miss, or a structure whose pieces differ, is solved
//! again from the source, and the whole directory can go at any time.

use std::path::Path;

use pxl_spec::bake::deck::Form;

use crate::deck::piece::Piece;
use crate::util::{pack, unpack, write_atomic, Level};

/// A structure's surface, a run a piece in the structure's order.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Profile {
    pub runs: Vec<Run>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Run {
    pub gers_id: String,
    pub start: u64,
    /// Each point: metres along the piece, x, y, the surface.
    pub points: Vec<[f64; 4]>,
    /// The ground under its first and last point.
    pub ground: [f64; 2],
}

impl Run {
    /// The surface `d` metres along.
    pub fn top_along(&self, d: f64) -> f64 {
        let p = &self.points;
        let i = p.partition_point(|q| q[0] < d).clamp(1, p.len().max(2) - 1);
        let (a, b) = (p[i - 1], p[i]);
        let t = if b[0] > a[0] { ((d - a[0]) / (b[0] - a[0])).clamp(0.0, 1.0) } else { 0.0 };
        a[3] + (b[3] - a[3]) * t
    }
}

/// A structure by its least piece, which is in no other; `held` apart from
/// the solve that leaves out what crosses it.
fn key(members: &[Piece], held: bool) -> String {
    let least = members.iter().map(|p| (p.gers_id.as_str(), p.start.to_bits())).min().expect("a structure has a piece");
    let form = members[0].form;
    format!("{}-{}-{}-{:016x}", if form == Form::Deck { "deck" } else { "bore" }, if held { "held" } else { "free" }, least.0, least.1)
}

fn same(members: &[Piece], profile: &Profile) -> bool {
    members.len() == profile.runs.len() && members.iter().zip(&profile.runs).all(|(p, r)| p.gers_id == r.gers_id && p.start.to_bits() == r.start)
}

pub fn read(dir: &Path, members: &[Piece], held: bool) -> Option<Profile> {
    let bytes = std::fs::read(dir.join(key(members, held))).ok()?;
    let profile: Profile = serde_json::from_slice(&unpack(&bytes).ok()?).ok()?;
    same(members, &profile).then_some(profile)
}

pub fn write(dir: &Path, members: &[Piece], held: bool, profile: &Profile) -> Result<(), String> {
    let packed = pack(&serde_json::to_vec(profile).map_err(|e| e.to_string())?, Level::Live).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    write_atomic(&dir.join(key(members, held)), &packed).map_err(|e| e.to_string())
}
