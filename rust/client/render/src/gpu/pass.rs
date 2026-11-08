// pxl-render/gpu/pass.rs

//! The passes that draw what the core lists over the world its rays cast (gpu/ray.rs): the
//! markers, the decor and its impostors, the location indicator and the characters. All but
//! the markers mark the tiles their changed draws cover (`mark`). The dev gizmos and the
//! character surface are passes of their own here as well.

pub mod character;
pub mod debug;
pub mod decor;
pub mod impostor;
pub mod location;
pub mod mark;
pub mod marker;
pub mod model;
pub mod skinned_mesh;
