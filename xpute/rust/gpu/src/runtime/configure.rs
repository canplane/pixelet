// xpute-gpu/runtime/configure.rs

/// This says where a stale surface is configured. Configuring a surface sets
/// its layer's properties, which some platforms allow on one thread only.
#[derive(Clone, Copy)]
pub enum ConfigureAt {
    /// The surface is configured on the thread that runs the stream.
    Here,
    /// The surface is configured wherever the given function sends the work,
    /// which must then run `configure_surfaces`.
    Elsewhere(fn()),
    /// The surface is configured on the main queue, where Core Animation sets a
    /// layer's properties. The given function must run `configure_surfaces`
    /// while nothing else uses the runtime; beside a native core, that is
    /// inside the core's `while_idle`. Asking twice before it runs configures
    /// nothing twice, since only a stale surface is configured.
    #[cfg(target_vendor = "apple")]
    Main(fn()),
}

impl ConfigureAt {
    pub(super) fn ask(self) {
        match self {
            ConfigureAt::Here => {}
            ConfigureAt::Elsewhere(ask) => ask(),
            #[cfg(target_vendor = "apple")]
            ConfigureAt::Main(configure) => on_main(configure),
        }
    }
}

/// Asks the main queue to run `f` once, and returns at once.
#[cfg(target_vendor = "apple")]
fn on_main(f: fn()) {
    use core::ffi::c_void;
    unsafe extern "C" {
        /// The queue `dispatch_get_main_queue()` returns. That function is an
        /// inline in a header, so Rust cannot call it.
        static _dispatch_main_q: c_void;
        fn dispatch_async_f(
            queue: *const c_void,
            context: *mut c_void,
            work: extern "C" fn(*mut c_void),
        );
    }
    extern "C" fn work(context: *mut c_void) {
        // SAFETY: the `fn()` `on_main` was given, carried as the context.
        let f = unsafe { core::mem::transmute::<*mut c_void, fn()>(context) };
        f();
    }
    // SAFETY: libdispatch's main queue, which lives as long as the process;
    // the context is a function's address, which `work` alone reads.
    unsafe {
        dispatch_async_f(
            &raw const _dispatch_main_q,
            core::ptr::with_exposed_provenance_mut(f as usize),
            work,
        )
    };
}
