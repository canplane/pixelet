// xpute-gpu/host/configure.rs

/// Where a stale surface is configured: configuring one sets its layer's
/// properties, which some platforms allow on one thread alone.
#[derive(Clone, Copy)]
pub enum ConfigureAt {
    /// On the thread that runs the stream.
    Here,
    /// Wherever `ask` sends it, which then runs `configure_surfaces`.
    Elsewhere(fn()),
    /// On the main queue, where Core Animation sets a layer's properties:
    /// `configure` must run `configure_surfaces` while nothing else uses the
    /// host (beside a native guest, inside its `while_idle`). Asked twice
    /// before it runs, it configures nothing twice: only a stale surface is.
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
        /// What `dispatch_get_main_queue()`, a header's inline, returns.
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
