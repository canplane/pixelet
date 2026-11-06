// pxl-kernel/sys/mem.rs

//! Which sections the core memory is cut into, and what `.fixed` holds:
//! ranges placed once at boot, never freed, sized at compile time
//! (xpute_core::mem::range). Only the kernel's own formats belong there;
//! external ones (glTF, PNG) have no predictable bound and are read into the
//! heap, a model kept parsed and an image only until its texture lands
//! (sys/io/loader.rs). Which ranges there are and how large is the roof's
//! (door/layout.rs), which hands each owner its own at boot.

pub(crate) mod section;
