# Indicators (the one-off indicator tier)

An indicator is one bespoke shape placed in the local world frame
(`../../view/frame/coord.rs`), tied to input or the device's state rather than
to anything the world keeps, and it does **not** face the camera. That last
trait is what separates it from a world-anchored billboard: the ghost box
(`ghost.rs`) and the location puck's zone ring and beacon (`location_puck.rs`)
sit and turn in the world's own space, and neither counter-rotates toward the
eye. Their colors are this tier's own, pxl-render `look/color.rs` `indicator`
(`ACTION`, `ERASE`), not a general palette.
