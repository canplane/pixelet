// pxl-render/gpu/pass.rs

//! The passes the renderer runs that are not the world itself: the
//! billboards, the decor, the location puck and the skinned actors. Each
//! takes the draws the scheduler listed for it and executes them; none
//! decides what is in its list.

pub mod badges;
pub mod decor;
pub mod impostor;
pub mod model;
pub mod puck;
pub mod skinned;
