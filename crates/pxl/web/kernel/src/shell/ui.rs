// pxl-kernel/shell/ui.rs

//! What a widget on the page reads of the kernel: whether the compass shows
//! and what its reset does, and how many meters the scale bar's bar is
//! worth. The drawing is
//! the shell's; the answer is the kernel's.

pub mod compass;
pub mod scale_bar;
