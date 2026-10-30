// pxl-spec/dirs.rs

//! The project's own folders on a machine, as XDG's base directories name a
//! machine's and the `directories` crate's `ProjectDirs` an application's
//! within them: each under the project's name (`APP`), so what a run leaves
//! there is found and cleared as one, and the name is said once.

use std::path::PathBuf;

/// The folder every one of them is under.
pub const APP: &str = "pixelet";

/// `of` in the project's folder of the user's caches: macOS's
/// `~/Library/Caches`, else `$XDG_CACHE_HOME`, or `~/.cache` where it is
/// unset. Kept across runs and checkouts, and safe to clear.
pub fn cache_dir(of: &str) -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("no $HOME")?;
    let caches = if cfg!(target_os = "macos") {
        PathBuf::from(home).join("Library/Caches")
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(home).join(".cache"))
    };
    Ok(caches.join(APP).join(of))
}

/// `of` in the project's folder of the system's temporary files; made by
/// whoever writes there.
pub fn temp_dir(of: &str) -> PathBuf {
    std::env::temp_dir().join(APP).join(of)
}
