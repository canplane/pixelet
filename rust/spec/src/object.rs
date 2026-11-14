// pxl-spec/object.rs

//! What the object layer stores in a cell: the asset placed there, by its id, which is the
//! kind over the index (`asset/id.json`). A cube's id is its value (value.rs), so a cube in a
//! palette entry stores the entry's number. The decor kind is the decors
//! (`asset/decor.json`), and the marker kind the pin. `ext` is what a placed asset carries
//! beyond its id.

pub mod ext;
