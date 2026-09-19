// pxl-kernel/shell/ui.rs

//! What a widget on the page reads of the kernel: whether the compass shows
//! and what its reset does, the minimap's image of the pages around the
//! centre, and how many metres the scale bar's bar is worth. The drawing is
//! the shell's; the answer is the kernel's.

pub mod compass;
pub mod minimap_atlas;
pub mod scale_bar;
