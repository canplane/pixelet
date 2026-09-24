// pxl-spec/poi.rs

pub mod detail;
pub mod wire;
pub mod xy;

/// An entity's source observations are owned by acquisition, not its spatial
/// representation. Text stays with this record; summaries contain no text.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Entity {
    pub id: String,
    /// Absolute canonical units on all three axes, not page-local offsets.
    pub position: [f64; 3],
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Summary {
    pub key: u64,
    pub count: u64,
    /// Conservative entity bounds, min XYZ then max XYZ. Not a label anchor.
    pub bounds: [f64; 6],
    pub refinable: bool,
}

/// A complete refinement for one node in an immutable bake snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub revision: u64,
    pub summary: Summary,
    pub children: Vec<Summary>,
}
