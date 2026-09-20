// pxl-spec/cell.rs

//! The cell model: the generated tables — colours, cubes, cube types, decor,
//! their types — and `object`, where a cell becomes the value the object
//! store keeps.

#[path = "cell/color.spec.rs"]
pub mod color;
pub mod cube;
#[path = "cell/cube_type.spec.rs"]
pub mod cube_type;
#[path = "cell/decor.spec.rs"]
pub mod decor;
pub mod object;
pub mod r#type;

pub use cube::{cube_colors, cube_types, CubeColorId};
pub use decor::{decor_types, DecorId};
