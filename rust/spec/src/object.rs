// pxl-spec/object.rs

//! What the object layer stores in a cell: the asset placed there, by its asset id, which is
//! the kind over the index (`asset/id.json`). A cube's asset id is its value (value.rs), so a
//! placed ground cube stores its own asset id. The decor kind is the decors
//! (`asset/decor.json`), and the marker kind the pin. `ext` is what a placed asset carries
//! beyond its asset id.

pub mod ext;
