// pxl-kernel/shell.rs

//! What the page's UI reads of the kernel and what it asks of it: the one
//! state it reads, the settings it changes and keeps, and the map it drives.
//! The words a person sees are the shell's (apps/web); what is here is what
//! they are drawn from.

pub mod map_store;
pub mod setting;
#[allow(clippy::module_inception)]
pub mod shell;
pub mod ui;
